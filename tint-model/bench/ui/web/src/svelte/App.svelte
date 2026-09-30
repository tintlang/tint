<script>
  let nextId = 0;
  const make = (n) => { const out = new Array(n); for (let i = 0; i < n; i++) { nextId++; out[i] = `${nextId} item ${nextId}`; } return out; };
  let rows = $state.raw([]);
  let selected = $state('');
  function swap() { const c = rows.slice(); const n = c.length; [c[1], c[n - 2]] = [c[n - 2], c[1]]; rows = c; }
</script>

<div>
  <div class="bar">
    <button onclick={() => (rows = make(1000))}><span>Create 1,000 rows</span></button>
    <button onclick={() => (rows = make(10000))}><span>Create 10,000 rows</span></button>
    <button onclick={() => (rows = rows.concat(make(1000)))}><span>Append 1,000 rows</span></button>
    <button onclick={() => (rows = rows.map((t, i) => (i % 10 === 0 ? t + ' !!!' : t)))}><span>Update every 10th row</span></button>
    <button onclick={swap}><span>Swap rows</span></button>
    <button onclick={() => (selected = rows[4])}><span>Select row</span></button>
    <button onclick={() => (rows = rows.filter((_, i) => i !== 4))}><span>Remove row</span></button>
    <button onclick={() => (rows = [])}><span>Clear</span></button>
  </div>
  <div class="table">
    {#each rows as t (t)}
      <div class="line" class:sel={t === selected}>
        <div class="cell"><span>{t}</span></div>
        <button><span>x</span></button>
      </div>
    {/each}
  </div>
</div>
