// The "realistic" React stack: React + react-router + Tailwind (compiled, purged CSS file).
// Same DOM shape and behaviour as ../react/main.jsx; only the page chrome (router, nav) differs.
import React, { useState, memo } from 'react';
import { createRoot } from 'react-dom/client';
import { HashRouter, Routes, Route, Link } from 'react-router-dom';
import './index.css';

let nextId = 0;
const make = (n) => { const out = new Array(n); for (let i = 0; i < n; i++) { nextId++; out[i] = `${nextId} item ${nextId}`; } return out; };

const Line = memo(function Line({ text, sel }) {
  return (
    <div className={sel ? 'flex flex-row gap-2 bg-[#6c5ce7]' : 'flex flex-row gap-2 bg-[#1b1e26]'}>
      <div className="grow"><span>{text}</span></div>
      <button><span>x</span></button>
    </div>
  );
});

function Bench() {
  const [rows, setRows] = useState([]);
  const [selected, setSelected] = useState('');
  return (
    <div className="flex flex-col">
      <div className="flex flex-row gap-1.5">
        <button onClick={() => setRows(make(1000))}><span>Create 1,000 rows</span></button>
        <button onClick={() => setRows(make(10000))}><span>Create 10,000 rows</span></button>
        <button onClick={() => setRows((r) => r.concat(make(1000)))}><span>Append 1,000 rows</span></button>
        <button onClick={() => setRows((r) => r.map((t, i) => (i % 10 === 0 ? t + ' !!!' : t)))}><span>Update every 10th row</span></button>
        <button onClick={() => setRows((r) => { const c = r.slice(); const n = c.length; [c[1], c[n - 2]] = [c[n - 2], c[1]]; return c; })}><span>Swap rows</span></button>
        <button onClick={() => setSelected(rows[4])}><span>Select row</span></button>
        <button onClick={() => setRows((r) => r.filter((_, i) => i !== 4))}><span>Remove row</span></button>
        <button onClick={() => setRows([])}><span>Clear</span></button>
      </div>
      <div className="flex flex-col" data-tag="Table">
        {rows.map((t) => <Line key={t} text={t} sel={t === selected} />)}
      </div>
    </div>
  );
}

function About() { return <p className="p-4 text-sm">About</p>; }

function App() {
  return (
    <HashRouter>
      <nav className="flex gap-4 p-2 text-sm"><Link to="/">Bench</Link><Link to="/about">About</Link></nav>
      <Routes><Route path="/" element={<Bench />} /><Route path="/about" element={<About />} /></Routes>
    </HashRouter>
  );
}
createRoot(document.getElementById('app')).render(<App />);
