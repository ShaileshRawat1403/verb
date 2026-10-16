// I-5 emulation-cost probe: integer loop plus JSON round-trips, a crude stand-in for tooling work.
const t0 = process.hrtime.bigint();
let s = 0;
for (let i = 0; i < 3e7; i++) s = (s + i * 7) % 1000003;
let o = { a: [] };
for (let i = 0; i < 2e4; i++) o.a.push({ i, t: "x".repeat(16) });
for (let k = 0; k < 20; k++) o = JSON.parse(JSON.stringify(o));
const ms = Number(process.hrtime.bigint() - t0) / 1e6;
console.log(`node-bench ${process.version} jitless=${process.execArgv.includes("--jitless")} sum=${s} items=${o.a.length} ms=${ms.toFixed(0)}`);
