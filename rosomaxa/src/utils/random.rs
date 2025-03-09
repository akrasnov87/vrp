#[cfg(test)]
#[path = "../../tests/unit/utils/random_test.rs"]
mod random_test;

use crate::utils::Float;
use rand::Error;
use rand::prelude::*;
use rand_distr::{Gamma, Normal};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::sync::Arc;

/// Provides the way to sample from different distributions.
pub trait DistributionSampler {
    /// Returns a sample from gamma distribution.
    fn gamma(&self, shape: Float, scale: Float) -> Float;

    /// Returns a sample from normal distribution.
    fn normal(&self, mean: Float, std_dev: Float) -> Float;
}

/// Provides the way to use randomized values in generic way.
pub trait Random: Send + Sync {
    /// Produces integral random value, uniformly distributed on the closed interval [min, max]
    fn uniform_int(&self, min: i32, max: i32) -> i32;

    /// Produces real random value, uniformly distributed on the closed interval [min, max)
    fn uniform_real(&self, min: Float, max: Float) -> Float;

    /// Flips a coin and returns true if it is "heads", false otherwise.
    fn is_head_not_tails(&self) -> bool;

    /// Tests probability value in (0., 1.) range.
    fn is_hit(&self, probability: Float) -> bool;

    /// Returns an index from collected with probability weight.
    /// Uses exponential distribution where the weights are the rate of the distribution (lambda)
    /// and selects the smallest sampled value.
    fn weighted(&self, weights: &[usize]) -> usize;

    /// Returns RNG.
    fn get_rng(&self) -> RandomGen;
}

/// Provides way to sample from different distributions.
#[derive(Clone)]
pub struct DefaultDistributionSampler(Arc<dyn Random>);

impl DefaultDistributionSampler {
    /// Creates a new instance of `DefaultDistributionSampler`.
    pub fn new(random: Arc<dyn Random>) -> Self {
        Self(random)
    }

    /// Returns a sample from gamma distribution.
    pub fn sample_gamma(shape: Float, scale: Float, random: &dyn Random) -> Float {
        Gamma::new(shape, scale)
            .unwrap_or_else(|_| panic!("cannot create gamma dist: shape={shape}, scale={scale}"))
            .sample(&mut random.get_rng())
    }

    /// Returns a sample from normal distribution.
    pub fn sample_normal(mean: Float, std_dev: Float, random: &dyn Random) -> Float {
        Normal::new(mean, std_dev)
            .unwrap_or_else(|_| panic!("cannot create normal dist: mean={mean}, std_dev={std_dev}"))
            .sample(&mut random.get_rng())
    }
}

impl DistributionSampler for DefaultDistributionSampler {
    fn gamma(&self, shape: Float, scale: Float) -> Float {
        Self::sample_gamma(shape, scale, self.0.as_ref())
    }

    fn normal(&self, mean: Float, std_dev: Float) -> Float {
        Self::sample_normal(mean, std_dev, self.0.as_ref())
    }
}

/// A default random implementation.
#[derive(Default)]
pub struct DefaultRandom {
    use_repeatable: bool,
}

impl DefaultRandom {
    /// Creates an instance of `DefaultRandom` with repeatable (predictable) random generation.
    pub fn new_repeatable() -> Self {
        Self { use_repeatable: true }
    }
}

impl Random for DefaultRandom {
    fn uniform_int(&self, min: i32, max: i32) -> i32 {
        if min == max {
            return min;
        }

        assert!(min < max);
        self.get_rng().gen_range(min..max + 1)
    }

    fn uniform_real(&self, min: Float, max: Float) -> Float {
        if (min - max).abs() < Float::EPSILON {
            return min;
        }

        assert!(min < max);
        self.get_rng().gen_range(min..max)
    }

    fn is_head_not_tails(&self) -> bool {
        self.get_rng().gen_bool(0.5)
    }

    fn is_hit(&self, probability: Float) -> bool {
        #![allow(clippy::unnecessary_cast)]
        self.get_rng().gen_bool(probability.clamp(0., 1.) as f64)
    }

    fn weighted(&self, weights: &[usize]) -> usize {
        weights
            .iter()
            .zip(0_usize..)
            .map(|(&weight, index)| (-self.uniform_real(0., 1.).ln() / weight as Float, index))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .unwrap()
            .1
    }

    fn get_rng(&self) -> RandomGen {
        RandomGen { use_repeatable: self.use_repeatable }
    }
}

thread_local! {
    /// Random generator seeded from thread_rng to make runs non-repeatable.
    static RANDOMIZED_RNG: RefCell<SmallRng> = RefCell::new(SmallRng::from_rng(thread_rng()).expect("cannot get RNG from thread rng"));

    /// Random generator seeded with 0 SmallRng to make runs repeatable.
    static REPEATABLE_RNG: RefCell<SmallRng> = RefCell::new(SmallRng::seed_from_u64(0));
}

/// Provides underlying random generator API.
#[derive(Clone, Debug)]
pub struct RandomGen {
    use_repeatable: bool,
}

impl RandomGen {
    /// Creates an instance of `RandomGen` using random generator with fixed seed.
    pub fn new_repeatable() -> Self {
        Self { use_repeatable: true }
    }

    /// Creates an instance of `RandomGen` using random generator with randomized seed.
    pub fn new_randomized() -> Self {
        Self { use_repeatable: false }
    }
}

impl RngCore for RandomGen {
    fn next_u32(&mut self) -> u32 {
        // NOTE use 'likely!' macro for better branch prediction once it is stabilized?
        if self.use_repeatable {
            REPEATABLE_RNG.with(|t| t.borrow_mut().next_u32())
        } else {
            RANDOMIZED_RNG.with(|t| t.borrow_mut().next_u32())
        }
    }

    fn next_u64(&mut self) -> u64 {
        if self.use_repeatable {
            REPEATABLE_RNG.with(|t| t.borrow_mut().next_u64())
        } else {
            RANDOMIZED_RNG.with(|t| t.borrow_mut().next_u64())
        }
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        if self.use_repeatable {
            REPEATABLE_RNG.with(|t| t.borrow_mut().fill_bytes(dest))
        } else {
            RANDOMIZED_RNG.with(|t| t.borrow_mut().fill_bytes(dest))
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        if self.use_repeatable {
            REPEATABLE_RNG.with(|t| t.borrow_mut().try_fill_bytes(dest))
        } else {
            RANDOMIZED_RNG.with(|t| t.borrow_mut().try_fill_bytes(dest))
        }
    }
}

impl CryptoRng for RandomGen {}

/// Returns an index of max element in values. In case of many same max elements,
/// returns the one from them at random.
pub fn random_argmax<I>(values: I, random: &dyn Random) -> Option<usize>
where
    I: Iterator<Item = Float>,
{
    let mut rng = random.get_rng();
    let mut count = 0;
    values
        .enumerate()
        .max_by(move |(_, r), (_, s)| match r.total_cmp(s) {
            Ordering::Equal => {
                count += 1;
                if rng.gen_range(0..=count) == 0 { Ordering::Less } else { Ordering::Greater }
            }
            Ordering::Less => {
                count = 0;
                Ordering::Less
            }
            Ordering::Greater => Ordering::Greater,
        })
        .map(|(idx, _)| idx)
}
