impl Particle {
    pub(crate) fn translate_position(&mut self, delta: Vec3) {
        self.position += delta;
    }
}
