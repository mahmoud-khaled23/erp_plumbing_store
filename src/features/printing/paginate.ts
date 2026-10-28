export interface MeasuredRow {
  id: string;
  height: number;
  group: string;
  heading: number;
  keepTogether?: boolean;
}
/** Pack measured rows without splitting them, reserving each repeated table heading. */
export function paginate(rows: MeasuredRow[], capacity: number): string[][] {
  if (!Number.isFinite(capacity) || capacity <= 0) throw new Error('page-capacity');
  const pages: string[][] = [];
  let page: string[] = [],
    used = 0,
    group = '';
  for (const [index, row] of rows.entries()) {
    if (!Number.isFinite(row.height) || row.height < 0 || row.height + row.heading > capacity)
      throw new Error('row-too-tall');
    let needed = row.height + (row.group === group ? 0 : row.heading);
    let block = needed;
    if (row.keepTogether && row.group !== group) {
      for (let i = index + 1; i < rows.length && rows[i].group === row.group; i++)
        block += rows[i].height;
      // Very large sections may still flow; a normal invoice summary stays intact.
      if (block > capacity) block = needed;
    }
    if (page.length && used + block > capacity) {
      pages.push(page);
      page = [];
      used = 0;
      group = '';
      needed = row.height + row.heading;
    }
    page.push(row.id);
    used += needed;
    group = row.group;
  }
  if (page.length || !pages.length) pages.push(page);
  return pages;
}
