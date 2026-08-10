import fs from 'fs';
const s = fs.readFileSync('tests/rm6-idle.test.ts', 'utf8');
const lines = s.split('\n');
let d = 0;
let instr = null;
for (let i = 0; i < lines.length; i++) {
  if (i + 1 < 150 || i + 1 > 200) continue;
  let line = lines[i];
  line = line.replace(/\/\*[\s\S]*?\*\//g, '');
  for (let k = 0; k < line.length; k++) {
    const ch = line[k];
    if (instr) {
      if (ch === instr) { instr = null; continue; }
      if (instr === '`' && ch === '\\') { k++; continue; }
      continue;
    }
    if (ch === "'" || ch === '"' || ch === '`') { instr = ch; continue; }
    if (ch === '/' && line[k + 1] === '/') break;
    if (ch === '/' && line[k + 1] === '*') { k += 2; continue; }
    if (ch === '{') d++;
    if (ch === '}') d--;
  }
  console.log(i + 1 + ' | d=' + d + ' | ' + lines[i].trim().slice(0, 70));
}
