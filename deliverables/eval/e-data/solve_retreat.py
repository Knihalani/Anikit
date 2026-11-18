# Solve, per year, the retreat factor m so that the drawn ice area is proportional to extent.
# r(a) = 0.36*(1+shape(a))*(1 - m*w(a)); w weights the retreat toward the Siberian/Pacific side.
import math
E=[7.05,7.67,7.14,7.30,7.39,6.81,6.70,7.41,7.28,7.37,7.01,6.14,6.47,7.47,6.40,7.14,6.08,7.58,6.69,6.54,6.12,6.25,6.73,5.83,6.12,5.98,5.50,5.86,4.27,4.69,5.26,4.87,4.56,3.57,5.21,5.22,4.62,4.53,4.82,4.79,4.36,4.00,4.95,4.90,4.38,4.35,4.75,4.81]
EMAX=7.80  # reference extent that fills the full shape
def shape(a): return 1+0.10*math.sin(2*a+0.7)+0.07*math.sin(3*a+2.1)+0.045*math.sin(5*a+1.3)+0.025*math.sin(9*a+0.4)+0.012*math.sin(17*a+2.2)
def w(a): return 0.12+0.88*(0.5+0.5*math.cos(a-2.3))**1.6
N=2000
A=[2*math.pi*i/N for i in range(N)]
def area(m): return sum((shape(a)*(1-m*w(a)))**2 for a in A)
A0=area(0)
M=[]
for e in E:
    lo,hi=0.0,0.95
    for _ in range(50):
        mid=(lo+hi)/2
        if area(mid)/A0 > e/EMAX: lo=mid
        else: hi=mid
    M.append(lo)
print(",".join(f"{m:.4f}" for m in M))
