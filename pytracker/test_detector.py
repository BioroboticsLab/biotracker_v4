from biotracker import *

import numpy as np
import json
import math
import random

import asyncio
from grpclib.server import Server

class TestDetector(FeatureDetectorBase):
    async def detect_features(self, request: "DetectorRequest") -> "DetectorResponse":
        try:
            shared_img = SharedImage(request.image)
        except FileNotFoundError as e:
            raise grpclib.GRPCError(grpclib.const.Status.NOT_FOUND, repr(e))
        self.step += 1
        return DetectorResponse(features=self.features, skeleton=self.skeleton)

    async def set_config(
        self, component_configuration: "ComponentConfig"
    ) -> "Empty":
        self.skeleton = SkeletonDescriptor(
            edges=[ SkeletonEdge(source=0, target=1) ],
            node_names=[ "head", "center" ],
            front_index=0,
            center_index=1,
            id=0
        )
        self.step = 0
        self.features = await self.generate_features(100, self.step)
        return Empty()

    async def generate_features(self, n, step):
        features = []
        for _ in range(n):
            x = random.randint(300, 1600)
            y = random.randint(300, 1600)
            features.append(Feature(image_nodes = [
                SkeletonNode(x=x, y=y, score=1),
                SkeletonNode(x=x+30, y=y+30, score=1)],
                                    score=1))
        return Features(features=features)


async def main():
    heartbeat()
    addr, port = get_address_and_port()
    server = Server([TestDetector()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
