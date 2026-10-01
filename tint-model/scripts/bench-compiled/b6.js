const {chromium}=require("playwright");const serve=require("./serve.js");
const app=process.argv[2]||'bench.wasm';
const N=+(process.argv[3]||31);
(async()=>{const srv=await serve(__dirname,8180);const b=await chromium.launch();
const p=await b.newPage({viewport:{width:1280,height:800}});p.on("pageerror",e=>console.log("pageerror",e.message));
let last='';p.on("console",m=>{if(m.text().startsWith('TINT-TIMING'))last=m.text()});
await p.goto("http://localhost:8180/?app="+app+"&entry=App");await p.waitForFunction("window.__done");
const ms=async(op)=>p.evaluate(`(()=>{const t0=performance.now();window.session.dispatch('${op}');document.body.offsetHeight;return performance.now()-t0})()`);
const med=a=>a.sort((x,y)=>x-y)[a.length>>1];
const run=async(name,ops,n=31)=>{const t=[];for(let i=0;i<n;i++){t.push(await ms(ops[i%ops.length]))}console.log(name.padEnd(10),med(t).toFixed(2))};
for(const [name,reset,ops] of [['create1k',null,['create1k']],['select',0,['select5','select9']],['update',0,['update10th']],['swap',0,['swap']],['remove',0,['remove5']],['append1k',0,['append1k']],['create10k',null,['create10k']]]){
 if(reset!==null)await p.evaluate("window.session.dispatch('create1k')");
 if(name=='append1k'){const t=[];for(let i=0;i<9;i++){await p.evaluate("window.session.dispatch('create1k')");t.push(await ms('append1k'))}console.log(name.padEnd(10),med(t).toFixed(2));continue}
 await run(name,ops,name=='create10k'||name=='create1k'?7:N)}
await b.close();srv.close()})();
