fn generate_hints(title: &str, num_steps: usize) -> (String, Vec<String>) {
    fn blank_out_indices(s: &str, indices: &[usize]) -> String {
        s.chars().enumerate().map(|(i, c)| if indices.contains(&i) { '_' } else { c }).collect()
    }

    let mut indices_hidden = Vec::new();
    for (i, c) in title.chars().enumerate() {
        if c.is_alphanumeric() {
            indices_hidden.push(i);
        }
    }
    let all_blanked_out = blank_out_indices(title, &indices_hidden);

    let mut hints = Vec::new();

    let mut num_hidden = indices_hidden.len() as f32;
    let num_revealed_per_step = num_hidden / 2.0 / num_steps as f32;
    for _ in 0..num_steps {
        num_hidden -= num_revealed_per_step;
        while indices_hidden.len() as f32 > num_hidden {
            indices_hidden.remove(fastrand::usize(..indices_hidden.len()));
        }

        hints.push(blank_out_indices(title, &indices_hidden));
    }

    (all_blanked_out, hints)
}

pub struct Hints {
    current_hint: String,
    hints_at: std::collections::HashMap<u32, String>,
}

impl Hints {
    pub fn new(song_title: &str, round_time: u32) -> Self {
        let hints_at = (10..u32::min(round_time, 70)).step_by(10).rev();
        let (mut current_hint, hints) = generate_hints(song_title, hints_at.len());
        let mut hints_at = hints_at.zip(hints).collect::<std::collections::HashMap<_, _>>();
        Self { current_hint, hints_at }
    }

    /// Must be called sequentially
    pub fn hint_at(&mut self, timer: u32) -> String {
        if let Some(new_hint) = self.hints_at.remove(&timer) {
            self.current_hint = new_hint;
        }
        self.current_hint.clone()
    }
}
