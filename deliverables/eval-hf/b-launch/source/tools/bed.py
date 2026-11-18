import numpy as np, subprocess, wave
SR=48000
def load(p):
    raw=subprocess.run(['ffmpeg','-v','error','-i',p,'-f','f32le','-ac','2','-ar',str(SR),'-'],capture_output=True).stdout
    return np.frombuffer(raw,dtype=np.float32).reshape(-1,2).copy()
d='/tmp/work/railyard/assets/sfx/'
thump=load(d+'impact-bass-1.mp3')[:int(0.35*SR)]
env=np.exp(-np.linspace(0,6,len(thump)))[:,None]; thump*=env
click=load(d+'click-soft.mp3')
key=load(d+'key-press.mp3')
dur=12.8; out=np.zeros((int(dur*SR),2),np.float32)
def put(s,t,g):
    i=int(t*SR); n=min(len(s),len(out)-i)
    if n>0: out[i:i+n]+=s[:n]*g
beat=0.49
nb=int(dur/beat)+1
for b in range(nb):
    t=b*beat
    ramp=min(1,0.35+b/16)
    if b%2==0: put(thump,t,0.55*ramp)
    # double clack of wheels over a rail joint, every beat
    put(click,t+0.0,0.35*ramp)
    put(key,t+0.14,0.22*ramp)
    if b%4==3: put(click,t+0.245,0.25*ramp)
# fade out last 0.8s
f=int(0.8*SR); out[-f:]*=np.linspace(1,0,f)[:,None]
fi=int(0.05*SR); out[:fi]*=np.linspace(0,1,fi)[:,None]
pk=np.abs(out).max(); out*= (10**(-3/20))/pk
pcm=(out*32767).astype('<i2')
w=wave.open('/tmp/work/railyard/assets/bed.wav','wb'); w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR); w.writeframes(pcm.tobytes()); w.close()
print('ok',len(out)/SR)
