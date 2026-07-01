#![allow(dead_code)]
use std::collections::VecDeque;

pub struct RingBuffer {
    buffer: VecDeque<f32>,
    capacity: usize,
}

impl RingBuffer {
    pub fn new(capacity_secs: f32, sample_rate: u32) -> Self {
        let capacity = (capacity_secs * sample_rate as f32).ceil() as usize;
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            if self.buffer.len() >= self.capacity {
                self.buffer.pop_front();
            }
            self.buffer.push_back(s);
        }
    }

    pub fn drain(&mut self) -> Vec<f32> {
        self.buffer.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
    }
}
