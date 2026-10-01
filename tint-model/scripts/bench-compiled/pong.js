const {chromium}=require("playwright");const serve=require("./serve.js");
(async()=>{
const srv=await serve(__dirname,8190);const b=await chromium.launch();
const p=await b.newPage({viewport:{width:1280,height:800}});
p.on("pageerror",e=>console.log(" pageerror:",e.message));
await p.addInitScript(()=>{const raf=window.requestAnimationFrame.bind(window);window.__d=[];window.requestAnimationFrame=cb=>raf(t=>{const t0=performance.now();cb(t);window.__d.push(performance.now()-t0)})});
await p.goto("http://localhost:8190/?app=site.wasm&entry=Landing&js=copy.js");await p.waitForFunction("window.__done");
await p.evaluate("history.pushState({}, '', '/pong'); window.dispatchEvent(new PopStateEvent('popstate'))");
await p.waitForTimeout(500);await p.keyboard.press("Space");await p.evaluate("window.__d=[]");
await p.waitForTimeout(5000);
const d=await p.evaluate("window.__d");d.sort((a,b)=>a-b);
console.log("n",d.length,"mean",(d.reduce((a,b)=>a+b,0)/d.length).toFixed(2),"p50",d[d.length>>1].toFixed(2),"p95",d[Math.floor(d.length*.95)].toFixed(2),"nodes",await p.evaluate("document.querySelectorAll('#root *').length"));
await b.close();srv.close()})();
