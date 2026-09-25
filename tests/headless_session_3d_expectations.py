"""GPU-free regression tests for the 3D acceptance-state assertions."""
import json
import unittest

from headless_session_3d import assert_fixture_info, assert_initialized_scene
from headless_session_3d_keyboard import assert_keyboard_images, assert_keyboard_motion


class HeadlessSession3dExpectations(unittest.TestCase):
    def setUp(self):
        self.info = json.loads(
            """{
                "physical_width": 321,
                "physical_height": 181,
                "scale_factor": 1.5,
                "vertical_fov_radians": 0.7853981852531433,
                "near": 0.10000000149011612,
                "far": 100.0,
                "front_step_per_tick": 0.07999999821186066
            }"""
        )
        self.initial = {"virtual_millis": 0}
        self.frozen = json.loads(
            """{
                "ticks": 1,
                "virtual_millis": 0,
                "camera_translation": [0.0, 0.0, 8.0],
                "camera_rotation": [0.0, 0.0, 0.0, 1.0],
                "camera_vertical_fov_radians": 0.7853981852531433,
                "camera_aspect_ratio": 1.7734806537628174,
                "camera_near": 0.10000000149011612,
                "camera_far": 100.0,
                "target_physical_size": [321, 181],
                "target_scale_factor": 1.5,
                "back_translation": [-0.4000000059604645, 0.0, 0.0],
                "front_translation": [0.4000000059604645, 0.0, 2.0]
            }"""
        )

    def test_serialized_f32_values_satisfy_the_real_acceptance_assertions(self):
        assert_fixture_info(self.info)
        assert_initialized_scene(self.initial, self.frozen)

    def test_meaningfully_wrong_near_or_object_positions_still_fail(self):
        wrong_info = dict(self.info, near=0.1001)
        with self.assertRaises(AssertionError):
            assert_fixture_info(wrong_info)

        for field, position in (
            ("back_translation", [-0.39, 0.0, 0.0]),
            ("front_translation", [0.41, 0.0, 2.0]),
        ):
            wrong_state = dict(self.frozen, **{field: position})
            with self.subTest(field=field), self.assertRaises(AssertionError):
                assert_initialized_scene(self.initial, wrong_state)

    def test_keyboard_motion_preserves_the_fixed_scene_and_uses_f32_expectations(self):
        moved = dict(
            self.frozen,
            ticks=11,
            virtual_millis=200,
            front_translation=[1.2000000476837158, 0.0, 2.0],
        )
        released = dict(moved, ticks=12, virtual_millis=220)
        self.assertEqual(
            assert_keyboard_motion(self.initial, self.frozen, moved, released, self.info),
            20,
        )

        wrong = dict(moved, front_translation=[1.19, 0.0, 2.0])
        with self.assertRaises(AssertionError):
            assert_keyboard_motion(self.initial, self.frozen, wrong, released, self.info)

    def test_keyboard_pixel_regions_require_depth_reveal_and_moved_red_geometry(self):
        def image(red_left, red_right):
            rows = [bytearray(321 * 3) for _ in range(181)]

            def fill(color, left, top, right, bottom):
                for y in range(top, bottom):
                    for x in range(left, right):
                        rows[y][x * 3:x * 3 + 3] = color

            fill(b"\x00\x00\xff", 100, 40, 198, 141)
            fill(b"\xff\x00\x00", red_left, 59, red_right, 122)
            fill(b"\x00\xff\x00", 12, 12, 84, 48)
            return [bytes(row) for row in rows]

        initial = image(147, 209)
        moved = image(179, 242)
        assert_keyboard_images(initial, moved, moved)

        with self.assertRaises(AssertionError):
            assert_keyboard_images(initial, initial, initial)


if __name__ == "__main__":
    unittest.main()
