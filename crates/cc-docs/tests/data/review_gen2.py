# SPDX-License-Identifier: GPL-3.0-or-later
# Provenance: second hostile-PDF generator of the same review (million pages, self-using
# tiling pattern, 2 million fill operations). See review_gen.py. Copied unchanged below.
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

# P7
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
