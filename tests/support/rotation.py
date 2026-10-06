"""Fixture-specific quaternion expectations, not a second production implementation."""
import math


def rotate(q, v):
    """Apply an xyzw unit quaternion to a vector."""
    x, y, z, w = q
    vx, vy, vz = v
    tx, ty, tz = 2 * (y * vz - z * vy), 2 * (z * vx - x * vz), 2 * (x * vy - y * vx)
    return [vx + w * tx + y * tz - z * ty,
            vy + w * ty + z * tx - x * tz,
            vz + w * tz + x * ty - y * tx]


def tick_rotation(q, delta=(0.0, 0.0)):
    """Fixture oracle: world Y/X drag in PreUpdate, then 20 ms of Y spin."""
    def product(a, b):
        x, y, z, w = a
        vx, vy, vz, vw = b
        return [w * vx + x * vw + y * vz - z * vy,
                w * vy - x * vz + y * vw + z * vx,
                w * vz + x * vy - y * vx + z * vw,
                w * vw - x * vx - y * vy - z * vz]

    dx, dy = delta
    yaw = [0.0, math.sin(dx * 0.01), 0.0, math.cos(dx * 0.01)]
    pitch = [math.sin(dy * 0.01), 0.0, 0.0, math.cos(dy * 0.01)]
    spin = [0.0, math.sin(0.005), 0.0, math.cos(0.005)]
    return product(spin, product(pitch, product(yaw, q)))
