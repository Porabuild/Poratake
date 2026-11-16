import base64, json, subprocess
from pathlib import Path
from html.parser import HTMLParser
class Result(HTMLParser):
 def __init__(self): super().__init__(); self.on=False; self.value=''
 def handle_starttag(self,tag,attrs): self.on=tag=='pre' and dict(attrs).get('id')=='result'
 def handle_endtag(self,tag):
  if tag=='pre': self.on=False
 def handle_data(self,data):
  if self.on:self.value+=data
p=Result();p.feed(Path('/tmp/poratake-frame-isolate-output.html').read_text());images=json.loads(p.value)
def decode(path):return subprocess.check_output(['ffmpeg','-v','error','-i',str(path),'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-'])
for name,data in images.items():
 path=Path('/tmp/poratake-frame-isolate-'+name+'-chrome.png');path.write_bytes(base64.b64decode(data));a=decode('/tmp/poratake-frame-isolate-'+name+'.png');b=decode(path)
 d=[max(abs(a[i+j]-b[i+j]) for j in range(4)) for i in range(0,len(a),4)]
 print(name,'above2',sum(x>2 for x in d),'max',max(d),'sum',sum(d))
