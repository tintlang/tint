const fs=require('fs');const [,,task]=process.argv;
const rtBytes=fs.readFileSync(__dirname+'/out/rt.wasm'), appBytes=fs.readFileSync(`${__dirname}/out/${task}.wasm`);
let mem=null,out='';
const dec=new TextDecoder();
const str=(p,l)=>dec.decode(new Uint8Array(mem.buffer,p,l));
const imports={env:new Proxy({
  host_trap:(p,l)=>{throw new Error('trap: '+str(p,l))},
  host_print:(p,l,nl)=>{out+=str(p,l)+(nl?'\n':'')},
  host_now_ms:()=>Date.now(),
},{get:(t,k)=>t[k]??((...a)=>{throw new Error('missing import '+String(k))})})};
const t0=performance.now();
const rt=new WebAssembly.Instance(new WebAssembly.Module(rtBytes),imports);
mem=rt.exports.memory;
const t1=performance.now();
const app=new WebAssembly.Instance(new WebAssembly.Module(appBytes),{rt:rt.exports});
const t2=performance.now();
app.exports.main();
const t3=performance.now();
process.stdout.write(out);
console.error(`rt-init ${(t1-t0).toFixed(1)} app-init ${(t2-t1).toFixed(1)} main ${(t3-t2).toFixed(1)}`);
