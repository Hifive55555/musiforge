
# 波动方程求解

在长为 $L$ 的弦上，某一点振动方程为

$$y(t,x)=y(x)\sin(\omega t+\phi) . \tag{1}$$

由牛顿第二定律，得微分方程

$$
c^2 \frac{\partial^{2}y}{\partial x^{2}} - \frac{\partial^{2}y}{\partial t^{2}}=0 ,
\tag{2}
$$

where

$$c = \sqrt{\frac{T_0}{\mu_0}} .$$

将 $(1)$ 代入 $(2)$，消去 $t$ 得

$$
\frac{d^2y(x)}{dx^2}+\omega^2\frac{\mu_0}{T_0}y(x)=0 .
\tag{3}
$$

> 这是一个谐振子图像函数y（x）的二阶微分方程，这个方程加上边界条件可以筛选出振动允许的角频率。因为 $ω,μ_0,T_0$ 都是常数，所以这个微分方程的解是三角函数。用迪利克雷边界条件（翻译21），我们可以得到非平凡解：

$$
y_n(x)=A_n\sin\left(\frac{n\pi x}{L}\right),
\quad n=1,2,\ldots
\tag{4}
$$

将 $(4)$ 代入 $(3)$，消去 $x$ 得

$$
\omega_n=\frac{n\pi c}{L},
\quad n=1,2,\ldots
\tag{5}
$$

## 振幅求解

边界条件：$y(0,t)=y(L,t)=0$ ,初始条件：$y(x,0)=0,\quad y_t(x,0)=U\delta(x-\alpha L)$。其中L 为弦长，$\alpha$ 为击弦点离弦最近端点的距离与弦长之比，$U$ 为击弦点处的初速度。

解得

$$
y(t,x)=\sum_{n=1}^\infty A_n sin\left(\frac{\omega}{c}x\right) \sin(\omega t + \phi) .
\tag{6}
$$

其中

$$
A_n = 2U \frac{\sin(n\pi \alpha)}{n\pi c}
$$
