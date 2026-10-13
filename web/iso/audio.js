// Read authored SYLK cells. No measured audio paths/settings are embedded here.
export function soundRows(bytes) {
  const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  if (!text.startsWith('ID;')) throw new Error('SOUND_METADATA: expected SYLK ID');
  const cells = new Map(); let x = 1, y = 1;
  for (const line of text.split(/\r?\n/)) {
    if (!line.startsWith('C;')) continue;
    const fields = []; let field = '', quoted = false;
    for (let i = 0; i < line.length; i++) {
      const c = line[i];
      if (c === '"') {
        if (quoted && line[i + 1] === '"') { field += '""'; i++; continue; }
        quoted = !quoted;
      }
      if (c === ';' && !quoted) { fields.push(field); field = ''; }
      else field += c;
    }
    if (quoted) throw new Error('SOUND_METADATA: unterminated quote');
    fields.push(field);
    let value;
    for (const field of fields.slice(1)) {
      if (field[0] === 'X') x = Number(field.slice(1));
      else if (field[0] === 'Y') y = Number(field.slice(1));
      else if (field[0] === 'K') {
        const raw = field.slice(1);
        if (raw.startsWith('"') && raw.endsWith('"')) value = raw.slice(1, -1).replaceAll('""', '"');
        else if (/^-?\d+(\.\d+)?$/.test(raw)) value = Number(raw);
        else throw new Error(`SOUND_METADATA: unsupported cell value ${raw}`);
      }
    }
    if (!Number.isSafeInteger(x) || !Number.isSafeInteger(y) || x < 1 || y < 1 || x > 1024 || y > 100000)
      throw new Error('SOUND_METADATA: cell bounds');
    if (value !== undefined) {
      const key = `${x},${y}`;
      if (cells.has(key)) throw new Error(`SOUND_METADATA: duplicate cell ${key}`);
      cells.set(key, value);
    }
  }
  const headers = [...cells].filter(([key]) => key.endsWith(',1'));
  const rows = new Map();
  for (const [key, value] of cells) {
    const [cx, cy] = key.split(',').map(Number);
    if (cy === 1) continue;
    const name = headers.find(([h]) => Number(h.split(',')[0]) === cx)?.[1];
    if (typeof name !== 'string') continue;
    if (!rows.has(cy)) rows.set(cy, {});
    rows.get(cy)[name] = value;
  }
  return [...rows.values()];
}
export function clickDescriptor(bytes, metadataPath) {
  const matches = soundRows(bytes).filter(row => row.SoundName === 'GlueScreenClick');
  if (matches.length !== 1) throw new Error(`${metadataPath}: expected exactly one GlueScreenClick row`);
  const row = matches[0];
  if (typeof row.DirectoryBase !== 'string' || typeof row.FileNames !== 'string' || !row.FileNames || /[,;]/.test(row.FileNames))
    throw new Error(`${metadataPath}: unsupported/missing click file mapping`);
  if (!Number.isFinite(row.Volume) || row.Volume < 0 || row.Volume > 127) throw new Error(`${metadataPath}: unsupported/missing Volume`);
  if (row.Pitch !== 1) throw new Error(`${metadataPath}: unsupported click Pitch ${row.Pitch}`);
  if (row.Flags !== 'NODUPLICATES' && row.Flags !== 0) throw new Error(`${metadataPath}: unsupported click Flags ${row.Flags}`);
  return { path: `${row.DirectoryBase.replace(/[\\/]$/, '')}\\${row.FileNames}`,
    settings: { volume: row.Volume / 127, noDuplicates: row.Flags === 'NODUPLICATES' },
    provenance: { metadataPath, row: row.SoundName, authored: row } };
}
