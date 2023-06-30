from biotracker import *

import numpy as np
import json
import math

import asyncio
from grpclib.server import Server

class SLEAPTracker(FeatureDetectorBase):
    async def detect_features(self, request: "DetectorRequest") -> "DetectorResponse":
        try:
            shared_img = SharedImage(request.image)
        except FileNotFoundError as e:
            raise grpclib.GRPCError(grpclib.const.Status.NOT_FOUND, repr(e))
        features = Features(features=[])
        return DetectorResponse(features=features, skeleton=self.skeleton)

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
        return Empty()

async def main():
    heartbeat()
    addr, port = get_address_and_port()
    server = Server([SLEAPTracker()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
