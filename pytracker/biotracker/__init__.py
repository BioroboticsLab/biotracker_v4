from .shared_image import SharedImage, BufferManager
from .biotracker.biotracker import *
from grpclib.client import Channel

import asyncio
import os
import signal
import numpy as np
import sys

def get_address_and_port():
    import urllib.parse
    import os
    address = os.getenv('BIOTRACKER_COMPONENT_ADDRESS')
    assert(address is not None)
    # urlparse() and urlsplit() insists on absolute URLs starting with "//"
    result = urllib.parse.urlsplit('//' + address)
    return result.hostname, result.port

def heartbeat():
    async def poll_core():
        try:
            async with Channel("127.0.0.1", 27342) as channel:
                core = BioTrackerStub(channel)
                while True:
                    await core.heartbeat(Empty())
                    await asyncio.sleep(1)
        except Exception as e:
            os.kill(os.getpid(), signal.SIGKILL)
    asyncio.create_task(poll_core())

def feature_to_world_pose(feature: "Feature", skeleton: "SkeletonDescriptor") -> "Pose":
    front = feature.world_nodes[skeleton.front_index]
    center = feature.world_nodes[skeleton.center_index]
    midline = (front.x - center.x, front.y - center.y)
    direction = midline / np.linalg.norm(midline)
    orientation = np.arctan2(direction[0], direction[1]) + np.pi / 2.0
    if np.isnan(orientation):
        # happens if center == front
        orientation = 0.0
    return Pose(x=center.x, y=center.y, orientation=orientation)

def log(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)

