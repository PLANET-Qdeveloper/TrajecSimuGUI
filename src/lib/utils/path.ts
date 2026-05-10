export function toAbsolute(baseDir: string, relPath: string): string {
  if (!relPath) return '';
  if (relPath.startsWith('/')) return relPath;
  const parts = (baseDir + '/' + relPath).split('/').filter(Boolean);
  const resolved: string[] = [];
  for (const part of parts) {
    if (part === '..') resolved.pop();
    else if (part !== '.') resolved.push(part);
  }
  return '/' + resolved.join('/');
}

export function toRelative(fromDir: string, absPath: string): string {
  if (!absPath) return '';
  if (!fromDir) return absPath;
  const from = fromDir.replace(/\/$/, '').split('/').filter(Boolean);
  const abs = absPath.split('/').filter(Boolean);
  let common = 0;
  while (common < from.length && common < abs.length && from[common] === abs[common]) {
    common++;
  }
  const ups = from.length - common;
  const rest = abs.slice(common);
  return [...Array(ups).fill('..'), ...rest].join('/') || '.';
}

export function dirOf(filePath: string): string {
  const idx = filePath.lastIndexOf('/');
  return idx >= 0 ? filePath.slice(0, idx) : '';
}
