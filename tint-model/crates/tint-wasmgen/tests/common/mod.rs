//! Shared by the wasm backend tests: compile a module and run an entry with the
//! runtime module (`tint-wasmrt`) under wasmi.
#![allow(dead_code)]
use std::path::PathBuf;
use std::sync::OnceLock;
use tint_ir::typed::{Module, TyKind};
use wasmi::{Caller, Engine, Linker, Store};

pub struct Host {
    pub trap: Option<String>,
    pub out: String,
}

/// The runtime module, built once per test process.
pub fn rt_wasm() -> &'static [u8] {
    static RT: OnceLock<Vec<u8>> = OnceLock::new();
    RT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let target = std::env::var("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|_| root.join("target"));
        let status = std::process::Command::new(cargo)
            .current_dir(&root)
            .args(["build", "-q", "-p", "tint-wasmrt", "--features", "ui,test-natives", "--target", "wasm32-unknown-unknown", "--profile", "wasm"])
            .status()
            .expect("run cargo");
        assert!(status.success(), "building tint-wasmrt failed");
        std::fs::read(target.join("wasm32-unknown-unknown/wasm/tint_wasmrt.wasm")).expect("tint_wasmrt.wasm")
    })
}

fn read_str(c: &Caller<'_, Host>, ptr: i32, len: i32) -> String {
    let mem = c.get_export("memory").and_then(|e| e.into_memory()).expect("rt memory");
    let data = mem.data(c);
    String::from_utf8_lossy(&data[ptr as usize..(ptr + len) as usize]).into_owned()
}

pub fn run(module: &Module, entry: &str) -> Result<Result<String, String>, String> {
    let compiled = tint_wasmgen::compile(module, &[entry]).map_err(|e| e.to_string())?;
    let engine = Engine::default();
    let rt_mod = wasmi::Module::new(&engine, rt_wasm()).map_err(|e| format!("INVALID RT: {e}"))?;
    let app_mod = wasmi::Module::new(&engine, &compiled.wasm).map_err(|e| format!("INVALID WASM: {e}"))?;
    let mut store = Store::new(&engine, Host { trap: None, out: String::new() });
    let mut linker = <Linker<Host>>::new(&engine);
    linker
        .func_wrap("env", "host_trap", |mut c: Caller<'_, Host>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
            let msg = read_str(&c, ptr, len);
            c.data_mut().trap = Some(msg.clone());
            Err(wasmi::Error::new(msg))
        })
        .unwrap();
    linker
        .func_wrap("env", "host_print", |mut c: Caller<'_, Host>, ptr: i32, len: i32, nl: i32| {
            let s = read_str(&c, ptr, len);
            c.data_mut().out.push_str(&s);
            if nl != 0 {
                c.data_mut().out.push('\n');
            }
        })
        .unwrap();
    linker.func_wrap("env", "host_now_ms", || -> f64 { 1_700_000_000_000.0 }).unwrap();
    let rt = linker.instantiate_and_start(&mut store, &rt_mod).map_err(|e| format!("rt instantiate: {e}"))?;
    linker.instance(&mut store, "rt", rt).map_err(|e| format!("link: {e}"))?;
    let app = match linker.instantiate_and_start(&mut store, &app_mod) {
        Ok(a) => a,
        Err(e) => {
            return match store.data().trap.clone() {
                Some(t) => Ok(Err(t)),
                None => Err(format!("instantiate: {e}")),
            }
        }
    };
    let f = module.func(module.functions[entry]);
    if !f.params.is_empty() {
        return Err("entry has parameters".into());
    }
    let live = rt.get_typed_func::<(), i64>(&store, "rt_live").unwrap();
    let before = live.call(&mut store, ()).unwrap();
    let func = app.get_func(&store, entry).unwrap();
    let mut out = [wasmi::Val::I32(0)];
    if let Err(e) = func.call(&mut store, &[], &mut out) {
        return Ok(Err(store.data().trap.clone().unwrap_or_else(|| format!("wasm trap: {e}"))));
    }
    let bits: u64 = match &out[0] {
        wasmi::Val::I32(v) => *v as u32 as u64,
        wasmi::Val::I64(v) => *v as u64,
        wasmi::Val::F64(v) => f64::from(*v).to_bits(),
        other => return Err(format!("unexpected result {other:?}")),
    };
    let _ = TyKind::Unit;
    let render = rt.get_typed_func::<(u32, i64), u32>(&store, "render_value").unwrap();
    let p = match render.call(&mut store, (f.ret.0, bits as i64)) {
        Ok(p) => p as i32,
        Err(e) => return Ok(Err(store.data().trap.clone().unwrap_or_else(|| format!("render: {e}")))),
    };
    let data = rt.get_typed_func::<i32, i32>(&store, "str_data").unwrap();
    let len = rt.get_typed_func::<i32, i32>(&store, "str_bytes").unwrap();
    let (d, l) = (data.call(&mut store, p).unwrap(), len.call(&mut store, p).unwrap());
    let mem = rt.get_memory(&store, "memory").unwrap();
    let text = String::from_utf8_lossy(&mem.data(&store)[d as usize..(d + l) as usize]).into_owned();
    // Give back the rendered text and the result; nothing else may be left.
    let release = rt.get_typed_func::<i32, ()>(&store, "release").unwrap();
    release.call(&mut store, p).unwrap();
    if std::env::var("NO_RELEASE").is_err() && matches!(out[0], wasmi::Val::I32(_)) && !matches!(module.types.kind(f.ret), TyKind::Unit | TyKind::Bool) {
        release.call(&mut store, bits as i32).unwrap();
    }
    let after = live.call(&mut store, ()).unwrap();
    if after != before {
        return Err(format!("LEAK: {} objects left alive (text {text})", after - before));
    }
    Ok(Ok(text))
}


/// A running compiled app (runtime + app instance) that can be called repeatedly.
pub struct Session {
    pub store: Store<Host>,
    pub rt: wasmi::Instance,
    pub app: wasmi::Instance,
}

impl Session {
    pub fn new(module: &Module, entries: &[&str]) -> Result<Session, String> {
        Self::with_meta(module, entries, "")
    }

    pub fn with_meta(module: &Module, entries: &[&str], meta: &str) -> Result<Session, String> {
        Self::with_storage(module, entries, meta, "")
    }

    /// `storage_json` (a JSON object, or empty) is loaded before the app's globals are initialized.
    pub fn with_storage(module: &Module, entries: &[&str], meta: &str, storage_json: &str) -> Result<Session, String> {
        let compiled = tint_wasmgen::compile_with(module, entries, meta).map_err(|e| e.to_string())?;
        let engine = Engine::default();
        let rt_mod = wasmi::Module::new(&engine, rt_wasm()).map_err(|e| format!("INVALID RT: {e}"))?;
        let app_mod = wasmi::Module::new(&engine, &compiled.wasm).map_err(|e| format!("INVALID WASM: {e}"))?;
        let mut store = Store::new(&engine, Host { trap: None, out: String::new() });
        let mut linker = <Linker<Host>>::new(&engine);
        linker
            .func_wrap("env", "host_trap", |mut c: Caller<'_, Host>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
                let msg = read_str(&c, ptr, len);
                c.data_mut().trap = Some(msg.clone());
                Err(wasmi::Error::new(msg))
            })
            .unwrap();
        linker
            .func_wrap("env", "host_print", |mut c: Caller<'_, Host>, ptr: i32, len: i32, nl: i32| {
                let s = read_str(&c, ptr, len);
                c.data_mut().out.push_str(&s);
                if nl != 0 {
                    c.data_mut().out.push('\n');
                }
            })
            .unwrap();
        linker.func_wrap("env", "host_now_ms", || -> f64 { 1_700_000_000_000.0 }).unwrap();
        let rt = linker.instantiate_and_start(&mut store, &rt_mod).map_err(|e| format!("rt instantiate: {e}"))?;
        linker.instance(&mut store, "rt", rt).map_err(|e| format!("link: {e}"))?;
        if !storage_json.is_empty() {
            let mut pre = Session { store, rt, app: rt };
            let p = pre.new_str(storage_json);
            pre.rt.get_typed_func::<i32, ()>(&pre.store, "storage_hydrate_json").unwrap().call(&mut pre.store, p).unwrap();
            pre.rt.get_typed_func::<i32, ()>(&pre.store, "release").unwrap().call(&mut pre.store, p).unwrap();
            store = pre.store;
        }
        let app = linker
            .instantiate_and_start(&mut store, &app_mod)
            .map_err(|e| store.data().trap.clone().unwrap_or_else(|| format!("instantiate: {e}")))?;
        Ok(Session { store, rt, app })
    }

    /// Calls the exported function `name` without arguments.
    pub fn call(&mut self, name: &str) -> Result<(), String> {
        let f = self.app.get_func(&self.store, name).ok_or_else(|| format!("no export `{name}`"))?;
        let results = f.ty(&self.store).results().len();
        let mut out = vec![wasmi::Val::I32(0); results];
        f.call(&mut self.store, &[], &mut out)
            .map_err(|e| self.store.data().trap.clone().unwrap_or_else(|| format!("wasm trap: {e}")))
    }

    /// Renders what the runtime has collected; the JSON of the top-level nodes.
    pub fn render_json(&mut self) -> Result<String, String> {
        let f = self.rt.get_typed_func::<(), u32>(&self.store, "ui_render_json").unwrap();
        let p = f.call(&mut self.store, ()).map_err(|e| format!("render: {e}"))? as i32;
        let data = self.rt.get_typed_func::<i32, i32>(&self.store, "str_data").unwrap();
        let len = self.rt.get_typed_func::<i32, i32>(&self.store, "str_bytes").unwrap();
        let (d, l) = (data.call(&mut self.store, p).unwrap(), len.call(&mut self.store, p).unwrap());
        let mem = self.rt.get_memory(&self.store, "memory").unwrap();
        let text = String::from_utf8_lossy(&mem.data(&self.store)[d as usize..(d + l) as usize]).into_owned();
        let release = self.rt.get_typed_func::<i32, ()>(&self.store, "release").unwrap();
        release.call(&mut self.store, p).unwrap();
        Ok(text)
    }
}

impl Session {
    /// Writes `text` into a new runtime string and returns its pointer.
    pub fn new_str(&mut self, text: &str) -> i32 {
        let alloc = self.rt.get_typed_func::<i32, i32>(&self.store, "rt_alloc").unwrap();
        let at = alloc.call(&mut self.store, text.len() as i32).unwrap();
        let mem = self.rt.get_memory(&self.store, "memory").unwrap();
        mem.data_mut(&mut self.store)[at as usize..at as usize + text.len()].copy_from_slice(text.as_bytes());
        let make = self.rt.get_typed_func::<(i32, i32), i32>(&self.store, "str_new").unwrap();
        make.call(&mut self.store, (at, text.len() as i32)).unwrap()
    }

    /// Text of a runtime string (released afterwards).
    pub fn take_str(&mut self, p: i32) -> String {
        let data = self.rt.get_typed_func::<i32, i32>(&self.store, "str_data").unwrap();
        let len = self.rt.get_typed_func::<i32, i32>(&self.store, "str_bytes").unwrap();
        let (d, l) = (data.call(&mut self.store, p).unwrap(), len.call(&mut self.store, p).unwrap());
        let mem = self.rt.get_memory(&self.store, "memory").unwrap();
        let text = String::from_utf8_lossy(&mem.data(&self.store)[d as usize..(d + l) as usize]).into_owned();
        let release = self.rt.get_typed_func::<i32, ()>(&self.store, "release").unwrap();
        release.call(&mut self.store, p).unwrap();
        text
    }

    /// Calls a runtime export that returns a new string.
    pub fn rt_string(&mut self, name: &str) -> String {
        let f = self.rt.get_typed_func::<(), i32>(&self.store, name).unwrap();
        let p = f.call(&mut self.store, ()).unwrap();
        self.take_str(p)
    }
}

impl Session {
    /// Answers the host callback `id` that was left waiting: `Ok(())` or `Err(message)`.
    pub fn answer_callback(&mut self, id: i32, message: Option<&str>) -> Result<(), String> {
        let msg = message.map(|m| self.new_str(m)).unwrap_or(0);
        let out = self.rt.get_typed_func::<i32, i32>(&self.store, "rt_alloc").unwrap().call(&mut self.store, 8).unwrap();
        let f = self.rt.get_typed_func::<(i32, i32, i32, i32), i32>(&self.store, "callback_answer").unwrap();
        let found = f.call(&mut self.store, (id, message.is_none() as i32, msg, out)).unwrap();
        if found == 0 {
            return Err("no such callback".into());
        }
        let mem = self.rt.get_memory(&self.store, "memory").unwrap();
        let data = mem.data(&self.store);
        let read = |o: usize| i32::from_le_bytes(data[out as usize + o..out as usize + o + 4].try_into().unwrap());
        let (closure, result) = (read(0), read(4));
        let run = self.app.get_func(&self.store, "tint:run_callback").ok_or("no tint:run_callback")?;
        run.call(&mut self.store, &[wasmi::Val::I32(closure), wasmi::Val::I32(result)], &mut [])
            .map_err(|e| self.store.data().trap.clone().unwrap_or_else(|| format!("wasm trap: {e}")))?;
        self.rt.get_typed_func::<i32, ()>(&self.store, "release").unwrap().call(&mut self.store, closure).unwrap();
        Ok(())
    }
}
