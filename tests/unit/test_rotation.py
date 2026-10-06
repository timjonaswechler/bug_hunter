"""Headless checks of the mesh CLI test's quaternion oracle.

Run: python3 -m unittest discover -s tests -p 'test_mesh_rotation.py'
"""
import math
import unittest

from tests.support.rotation import rotate, tick_rotation


class MeshRotationTests(unittest.TestCase):
    def assert_vector(self, actual, expected):
        for a, b in zip(actual, expected):
            self.assertAlmostEqual(a, b, places=12)

    def test_idle_and_horizontal_drag_match_closed_form_yaw(self):
        prior = [0.0, math.sin(0.47 / 2), 0.0, math.cos(0.47 / 2)]
        for dx in (0.0, 12.0, -12.0):
            yaw = 0.47 + dx * 0.02 + 0.01
            self.assert_vector(tick_rotation(prior, (dx, 0.0)),
                               [0.0, math.sin(yaw / 2), 0.0, math.cos(yaw / 2)])

    def test_vertical_drag_uses_world_x_after_existing_yaw(self):
        # Y(90) maps X to -Z, then world X(90) maps -Z to Y.
        # The final timed Y rotation must leave that Y vector unchanged.
        half = math.sqrt(0.5)
        q = tick_rotation([0.0, half, 0.0, half], (0.0, math.pi / 0.04))
        self.assert_vector(rotate(q, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0])

    def test_diagonal_drag_precedes_timed_spin(self):
        # Y(90), X(90), Y(0.01): Z -> X -> X -> (cos, 0, -sin).
        quarter_turn_pixels = math.pi / 0.04
        q = tick_rotation([0.0, 0.0, 0.0, 1.0],
                          (quarter_turn_pixels, quarter_turn_pixels))
        self.assert_vector(rotate(q, [0.0, 0.0, 1.0]),
                           [math.cos(0.01), 0.0, -math.sin(0.01)])
        self.assertAlmostEqual(sum(value * value for value in q), 1.0, places=12)


if __name__ == "__main__":
    unittest.main()
