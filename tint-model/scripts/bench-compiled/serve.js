const http=require('http'),fs=require('fs'),path=require('path');
const types={'.js':'text/javascript','.wasm':'application/wasm','.html':'text/html'};
module.exports=(root,port)=>new Promise(r=>{const s=http.createServer((q,res)=>{const u=new URL(q.url,'http://x');let p=path.join(root,u.pathname==='/'?'page.html':u.pathname);fs.readFile(p,(e,d)=>{if(e){res.writeHead(404);res.end();return}res.writeHead(200,{'content-type':types[path.extname(p)]||'application/octet-stream'});res.end(d)})}).listen(port,()=>r(s))});
