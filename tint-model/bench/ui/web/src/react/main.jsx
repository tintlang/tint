import React, { useState, useCallback, memo } from 'react';
import { createRoot } from 'react-dom/client';

let nextId = 0;
const make = (n) => { const out = new Array(n); for (let i = 0; i < n; i++) { nextId++; out[i] = `${nextId} item ${nextId}`; } return out; };

const Line = memo(function Line({ text, sel }) {
  return (
    <div className={sel ? 'line sel' : 'line'}>
      <div className="cell"><span>{text}</span></div>
      <button><span>x</span></button>
    </div>
  );
});

function App() {
  const [rows, setRows] = useState([]);
  const [selected, setSelected] = useState('');
  return (
    <div>
      <div className="bar">
        <button onClick={() => setRows(make(1000))}><span>Create 1,000 rows</span></button>
        <button onClick={() => setRows(make(10000))}><span>Create 10,000 rows</span></button>
        <button onClick={() => setRows((r) => r.concat(make(1000)))}><span>Append 1,000 rows</span></button>
        <button onClick={() => setRows((r) => r.map((t, i) => (i % 10 === 0 ? t + ' !!!' : t)))}><span>Update every 10th row</span></button>
        <button onClick={() => setRows((r) => { const c = r.slice(); const n = c.length; [c[1], c[n - 2]] = [c[n - 2], c[1]]; return c; })}><span>Swap rows</span></button>
        <button onClick={() => setSelected(rows[4])}><span>Select row</span></button>
        <button onClick={() => setRows((r) => r.filter((_, i) => i !== 4))}><span>Remove row</span></button>
        <button onClick={() => setRows([])}><span>Clear</span></button>
      </div>
      <div className="table">
        {rows.map((t) => <Line key={t} text={t} sel={t === selected} />)}
      </div>
    </div>
  );
}
createRoot(document.getElementById('app')).render(<App />);
