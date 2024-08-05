use std::collections::VecDeque;

struct Solution;

impl Solution {
  pub fn max_sliding_window(nums: Vec<i32>, k: i32) -> Vec<i32> {
    if nums.len() == 0 || k == 1 {
      return nums;
    }

    let mut res: Vec<i32> = Vec::new();
    let mut deque: VecDeque<i32> = VecDeque::new();
    for i in 0 .. nums.len() {
      Self::push(&mut deque, nums[i]);

      if (i as i32) > k - 1 {
        Self::pop(&mut deque, nums[i - k as usize]);
        res.push(Self::max(&deque));
      } else if (i as i32) == k - 1 {
        res.push(Self::max(&deque));
      }
    }
    return res;
  }

  fn push(deque: &mut VecDeque<i32>, n: i32) {
    while !deque.is_empty() && *deque.back().unwrap() < n {
      deque.pop_back();
    }
    deque.push_back(n);
  }

  fn pop(deque: &mut VecDeque<i32>, n: i32) {
    if !deque.is_empty() && *deque.front().unwrap() == n {
      deque.pop_front();
    }
  }

  fn max(deque: &VecDeque<i32>) -> i32 {
    return *deque.front().unwrap();
  }
}

fn main() {
  // println!("Hello, world!");
  assert_eq!(
    Solution::max_sliding_window(vec![1, 3, -1, -3, 5, 3, 6, 7], 3),
    vec![3, 3, 5, 5, 6, 7]
  );
}
