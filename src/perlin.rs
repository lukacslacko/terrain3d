use statrs::distribution::{ContinuousCDF, Normal};
pub struct Perlin {
    pub seed: u32,
    pub frequency: f32,
    pub lacunarity: f32,
    pub persistence: f32,
    pub octaves: u32,
}

impl Perlin {
    pub fn noise(&self, x: f32, y: f32, z: f32) -> f32 {
        let mut total = 0.0;
        let mut frequency = self.frequency;
        let mut amplitude = self.persistence;

        for _ in 0..self.octaves {
            total += self.single_layer(x * frequency, y * frequency, z * frequency) * amplitude;
            frequency *= self.lacunarity;
            amplitude *= self.persistence;
        }

        total
    }

    fn single_layer(&self, x: f32, y: f32, z: f32) -> f32 {
        let x = x + self.seed as f32;
        let y = y + self.seed as f32;
        let z = z + self.seed as f32;

        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let zi = z.floor() as i32;

        let xf = x - xi as f32;
        let yf = y - yi as f32;
        let zf = z - zi as f32;

        let fade = |x: f32| 3.0 * x * x - 2.0 * x * x * x;
        let normdist = Normal::standard();
        let randvec = |h: [f64; 3]| {
            if h[0] < 0.0 || h[0] > 1.0 {
                panic!("h[0] out of bounds: {}", h[0]);
            }
            if h[1] < 0.0 || h[1] > 1.0 {
                panic!("h[1] out of bounds: {}", h[1]);
            }
            if h[2] < 0.0 || h[2] > 1.0 {
                panic!("h[2] out of bounds: {}", h[2]);
            }
            let (x, y, z) = (
                normdist.inverse_cdf(h[0]) as f32,
                normdist.inverse_cdf(h[1]) as f32,
                normdist.inverse_cdf(h[2]) as f32,
            );
            let r = (x * x + y * y + z * z).sqrt();
            (x / r, y / r, z / r)
        };
        let grad = |h: [f64; 3], x, y, z| {
            let (a, b, c) = randvec(h);
            a * x + b * y + c * z
        };

        let hash = |x: i32, y: i32, z: i32| {
            let mut a = x as u32;
            let mut b = y as u32;
            let mut c = z as u32;
            let mut res = [0.0, 0.0, 0.0];
            for coord in &mut res {
                for _ in 0..5 {
                    a = a.wrapping_add(0x9e3779b9);
                    b = b.wrapping_add(0x5e3719b9);
                    c = c.wrapping_add(0x7f4a7c15);
                    a = a.wrapping_mul(0x2bd1e995);
                    b = b.wrapping_mul(0x54d1e935);
                    c = c.wrapping_mul(0x2f1b7c15);
                    a ^= a >> 11;
                    b ^= b >> 13;
                    c ^= c >> 9;
                    (a, b, c) = (b, c, (a ^ b ^ c));
                }
                *coord = (a as f64) / u32::MAX as f64;
            }
            res
        };

        let aaa = hash(xi, yi, zi);
        let aba = hash(xi, yi + 1, zi);
        let baa = hash(xi + 1, yi, zi);
        let bba = hash(xi + 1, yi + 1, zi);
        let aab = hash(xi, yi, zi + 1);
        let abb = hash(xi, yi + 1, zi + 1);
        let bab = hash(xi + 1, yi, zi + 1);
        let bbb = hash(xi + 1, yi + 1, zi + 1);

        let u = fade(xf);
        let v = fade(yf);
        let w = fade(zf);

        let lerp = |t, a, b| a + t * (b - a);

        lerp(
            w,
            lerp(
                v,
                lerp(u, grad(aaa, xf, yf, zf), grad(baa, xf - 1.0, yf, zf)),
                lerp(
                    u,
                    grad(aba, xf, yf - 1.0, zf),
                    grad(bba, xf - 1.0, yf - 1.0, zf),
                ),
            ),
            lerp(
                v,
                lerp(
                    u,
                    grad(aab, xf, yf, zf - 1.0),
                    grad(bab, xf - 1.0, yf, zf - 1.0),
                ),
                lerp(
                    u,
                    grad(abb, xf, yf - 1.0, zf - 1.0),
                    grad(bbb, xf - 1.0, yf - 1.0, zf - 1.0),
                ),
            ),
        )
    }
}
