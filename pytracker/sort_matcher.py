from biotracker import *
import math
import numpy as np
from ioutrack import Sort
import asyncio
from grpclib.server import Server
import cv2

class SortMatcher(MatcherBase):
    async def match_features(
        self, request: "MatcherRequest"
    ) -> "Features":
        features = request.features
        boxes = []
        # calculate axis aligned bounding boxes for all features
        for f in features.features:
            if len(f.image_nodes) == 0:
                continue
            points = []
            for p in f.image_nodes:
                if math.isnan(p.x) or math.isnan(p.y):
                    continue
                points.append([p.x, p.y])
            points = np.array(points, dtype=np.int32)
            x, y, w, h = cv2.boundingRect(points)
            boxes.append([x, y, x + w, y + h, f.score])
        tracks = self.tracker.update(np.array(boxes))
        for i, f in enumerate(features.features):
            if tracks.shape[0] <= i:
                break
            f.id = int(tracks[i, 4])
        return request.features

    async def set_config(
        self, component_configuration: "ComponentConfig"
    ) -> "Empty":
        self.tracker = Sort(max_age=50, min_hits=3)
        return Empty()

async def main():
    heartbeat()
    addr, port = get_address_and_port()
    server = Server([SortMatcher()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
