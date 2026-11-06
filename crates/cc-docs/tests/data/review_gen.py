# SPDX-License-Identifier: GPL-3.0-or-later
# Provenance: written by the independent review of crates/cc-docs (2026-11-06) as its
# hostile-PDF generator (declared 2e9 x 2e9 image, 20000 x 20000 RGB image, 2 GiB flate
# bomb, xref with 4e9 entries, nested transparency groups, million pages, self-using
# tiling pattern). Copied unchanged below this header. NOT run by the tests: it writes
# files of up to 2 GB. The Rust tests tests/sandbox.rs, tests/poison.rs and
# tests/hostile.rs build the same shapes in memory at test time.
import zlib, sys, os
out = sys.argv[1] if len(sys.argv)>1 else 'p'
def build(objs, trailer_extra=""):
    o = bytearray(b"%PDF-1.4\n"); offs=[]
    for i,b in enumerate(objs):
        offs.append(len(o)); o += b"%d 0 obj\n"%(i+1) + b + b"\nendobj\n"
    x=len(o)
    o += b"xref\n0 %d\n0000000000 65535 f \n"%(len(objs)+1)
    for f in offs: o += b"%010d 00000 n \n"%f
    o += b"trailer\n<< /Size %d /Root 1 0 R %s>>\nstartxref\n%d\n%%%%EOF\n"%(len(objs)+1, trailer_extra.encode(), x)
    return bytes(o)
def stream(d, data):
    if isinstance(data,str): data=data.encode()
    return b"<< "+d.encode()+b" /Length %d >>\nstream\n"%len(data)+data+b"\nendstream"
def zeros(n):
    c = zlib.compressobj(9); chunk=bytes(1<<20); left=n; out=[]
    while left>0:
        k=min(left,len(chunk)); out.append(c.compress(chunk[:k])); left-=k
    out.append(c.flush()); return b"".join(out)
def page(extra, res, content, more):
    return build([b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
      ("<< /Type /Page /Parent 2 0 R %s /Contents 4 0 R /Resources << %s >> >>"%(extra,res)).encode(), stream("",content)]+more)
def w(n,b): open(os.path.join(out,n),'wb').write(b); print(n, len(b))
IMG="q 100 0 0 100 0 0 cm /Im Do Q"
# P1 declared 2e9 x 2e9 gray
w('img_2e9.pdf', page("/MediaBox [0 0 100 100]","/XObject << /Im 5 0 R >>",IMG.encode(),
  [stream("/Type /XObject /Subtype /Image /Width 2000000000 /Height 2000000000 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode", zeros(1<<20))]))
# P2 consistent 20000x20000 RGB image = 1.2 GB decoded, body 1.2GB zeros
z=zeros(20000*20000*3)
w('img_20k_rgb.pdf', page("/MediaBox [0 0 100 100]","/XObject << /Im 5 0 R >>",IMG.encode(),
  [stream("/Type /XObject /Subtype /Image /Width 20000 /Height 20000 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode", z)]))
z=zeros(10000*10000)
w('img_10k_gray.pdf', page("/MediaBox [0 0 100 100]","/XObject << /Im 5 0 R >>",IMG.encode(),
  [stream("/Type /XObject /Subtype /Image /Width 10000 /Height 10000 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode", z)]))
# P3 flate bomb content stream 2 GiB
z=zeros(2<<30)
b=build([b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
 b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>", stream("/Filter /FlateDecode", z)])
w('flate_2g.pdf', b)
# P3b flate bomb in image with Width 1 Height 1 (decoder output unbounded?)
w('flate_img_1x1.pdf', page("/MediaBox [0 0 100 100]","/XObject << /Im 5 0 R >>",IMG.encode(),
  [stream("/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode", z)]))
# P4 xref huge subsection
b=page("/MediaBox [0 0 100 100]","",b"",[])
b=b.replace(b"xref\n0 5\n", b"xref\n0 4000000000\n")
w('xref_huge.pdf', b)
# P8 nested transparency groups
N=45
objs=[b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
 b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << /XObject << /F 5 0 R >> /ExtGState << /G << /ca 0.5 /CA 0.5 >> >> >> >>",
 stream("","/G gs /F Do")]
for i in range(N):
    nxt = "%d 0 R"%(6+i) if i<N-1 else None
    res = "/XObject << /F %s >>"%nxt if nxt else ""
    cont = b"/G gs 1 0 0 rg 0 0 100 100 re f " + (b"/F Do" if nxt else b"")
    objs.append(stream("/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Group << /S /Transparency /I true >> /Resources << %s /ExtGState << /G << /ca 0.5 /CA 0.5 >> >> >>"%res, cont))
w('nested_groups.pdf', build(objs))
# P7 million pages
n=1000000
objs=[b"<< /Type /Catalog /Pages 2 0 R >>", ("<< /Type /Pages /Kids [%s] /Count %d >>"%(" ".join("%d 0 R"%(i+3) for i in range(n)),n)).encode()]
for i in range(n): objs.append(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>")
w('million_pages.pdf', build(objs))
# self-referencing tiling pattern
objs=[b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
 b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << /Pattern << /P 5 0 R >> >> >>",
 stream("","/Pattern cs /P scn 0 0 100 100 re f"),
 stream("/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 5 5] /XStep 5 /YStep 5 /Resources << /Pattern << /P 5 0 R >> >>","/Pattern cs /P scn 0 0 5 5 re f")]
w('pattern_self.pdf', build(objs))
# CPU: huge number of big fills with blend / clip at big size; 5MB content of 'q W n 0 0 1e4 1e4 re f Q'
c=b"0.5 g 0 0 100 100 re f\n"*2000000
w('many_fills.pdf', page("/MediaBox [0 0 100 100]","",zlib.compress(c) ,[]).replace(b"/Contents 4 0 R",b"/Contents 4 0 R"))
