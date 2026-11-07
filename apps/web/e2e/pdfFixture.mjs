// SPDX-License-Identifier: GPL-3.0-or-later
//
// A generated PDF for the end-to-end tests (no binary fixture in the repository).

/**
 * A small PDF with a correct cross-reference table. Page 1: a black box (top left) on a 300 x 200 pt page;
 * page 2: a red box (bottom right) on a 200 x 300 pt page; page 3: empty (white). Page 1 also carries a link
 * annotation (URI action) over the black box, which a viewer must not follow without a click, and which this
 * viewer does not expose at all.
 */
export function makePdf() {
  const streams = [
    '0 0 0 rg 20 100 80 80 re f',
    '1 0 0 rg 100 20 80 80 re f',
    '1 1 1 rg 0 0 1 1 re f',
  ]
  const objs = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 6 0 R /Annots [9 0 R] >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Contents 7 0 R >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 8 0 R >>',
    ...streams.map((s) => `<< /Length ${s.length} >>\nstream\n${s}\nendstream`),
    '<< /Type /Annot /Subtype /Link /Rect [20 100 100 180] /Border [0 0 0] /A << /S /URI /URI (https://example.org/from-pdf) >> >>',
  ]
  let out = '%PDF-1.4\n'
  const offsets = []
  objs.forEach((o, i) => {
    offsets.push(out.length)
    out += `${i + 1} 0 obj\n${o}\nendobj\n`
  })
  const xref = out.length
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n`
  for (const o of offsets) out += `${String(o).padStart(10, '0')} 00000 n \n`
  out += `trailer\n<< /Root 1 0 R /Size ${objs.length + 1} >>\nstartxref\n${xref}\n%%EOF\n`
  return Buffer.from(out, 'latin1')
}
