let nextId = 0;
const app = document.getElementById('app');
app.innerHTML = `<div><div class="bar"></div><div class="table"></div></div>`;
const bar = app.querySelector('.bar'), table = app.querySelector('.table');
let rows = []; // { text, el }
let selected = null;
function btn(label, fn) { const b = document.createElement('button'); b.innerHTML = `<span>${label}</span>`; b.onclick = fn; bar.appendChild(b); }
function line(text) {
  const el = document.createElement('div'); el.className = 'line';
  el.innerHTML = '<div class="cell"><span></span></div><button><span>x</span></button>';
  el.firstChild.firstChild.textContent = text;
  return { text, el };
}
function make(n) { const out = new Array(n); for (let i = 0; i < n; i++) { nextId++; out[i] = line(`${nextId} item ${nextId}`); } return out; }
function mountAll() { const f = document.createDocumentFragment(); for (const r of rows) f.appendChild(r.el); table.replaceChildren(f); }
btn('Create 1,000 rows', () => { rows = make(1000); mountAll(); });
btn('Create 10,000 rows', () => { rows = make(10000); mountAll(); });
btn('Append 1,000 rows', () => { const add = make(1000); rows = rows.concat(add); const f = document.createDocumentFragment(); for (const r of add) f.appendChild(r.el); table.appendChild(f); });
btn('Update every 10th row', () => { for (let i = 0; i < rows.length; i += 10) { rows[i].text += ' !!!'; rows[i].el.firstChild.firstChild.textContent = rows[i].text; } });
btn('Swap rows', () => { const n = rows.length; const a = rows[1], b = rows[n - 2]; rows[1] = b; rows[n - 2] = a; const ref = b.el.nextSibling === a.el ? b.el : b.el.nextSibling; const aNext = a.el.nextSibling; table.insertBefore(b.el, aNext); table.insertBefore(a.el, ref); });
btn('Select row', () => { if (selected) selected.el.classList.remove('sel'); selected = rows[4]; selected.el.classList.add('sel'); });
btn('Remove row', () => { const [r] = rows.splice(4, 1); r.el.remove(); });
btn('Clear', () => { rows = []; table.replaceChildren(); });
