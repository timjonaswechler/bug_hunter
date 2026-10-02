"""PNG validation and RGB decoding shared by scene tests and diagnostics."""
import struct
import zlib


def read_png(path):
    """Check the complete PNG, including CRCs and decompressible RGB scanlines."""
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    offset, compressed = 8, bytearray()
    width = height = None
    while offset < len(data):
        length = struct.unpack(">I", data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + length]
        crc = struct.unpack(">I", data[offset + 8 + length:offset + 12 + length])[0]
        assert zlib.crc32(kind + payload) == crc, (path, kind)
        offset += length + 12
        if kind == b"IHDR":
            width, height, bits, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload)
            assert (bits, color, compression, filtering, interlace) == (8, 2, 0, 0, 0)
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            assert offset == len(data)
            break
    else:
        raise AssertionError("missing PNG end")
    pixels = zlib.decompress(compressed)
    assert len(pixels) == height * (1 + width * 3)
    return width, height, pixels


def rgb_pixels(path):
    width, height, scanlines = read_png(path)
    stride = width * 3
    previous = bytearray(stride)
    image = []
    for row in range(height):
        offset = row * (stride + 1)
        kind = scanlines[offset]
        assert 0 <= kind <= 4, kind
        decoded = bytearray(scanlines[offset + 1:offset + 1 + stride])
        for i in range(stride):
            left = decoded[i - 3] if i >= 3 else 0
            up = previous[i]
            corner = previous[i - 3] if i >= 3 else 0
            if kind == 0:
                predictor = 0
            elif kind == 1:
                predictor = left
            elif kind == 2:
                predictor = up
            elif kind == 3:
                predictor = (left + up) // 2
            else:
                p = left + up - corner
                predictor = min((left, up, corner), key=lambda value: abs(p - value))
            decoded[i] = (decoded[i] + predictor) & 255
        image.append(decoded)
        previous = decoded
    return width, height, image
