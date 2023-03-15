from .shared_image import SharedImage, BufferManager
from .biotracker.biotracker import *
from grpclib.client import Channel

import asyncio
import os
import signal

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
