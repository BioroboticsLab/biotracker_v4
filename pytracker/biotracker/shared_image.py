from .protocol import ImageData
from multiprocessing.shared_memory import SharedMemory, resource_tracker
import numpy as np

class SharedImage:
    def __init__(self, msg: ImageData):
        self.size = msg.width * msg.height * 4
        self.shm = SharedMemory(msg.shm_id, size=self.size, create=False)
        # Don't track this memory, it gets cleaned up by the BioTracker
        resource_tracker.unregister(self.shm._name, 'shared_memory')
        self.ndarray = np.ndarray((msg.height, msg.width, 4), dtype=np.uint8, buffer=self.shm.buf)
        self.ndarray.flags.writeable = False

    def as_numpy(self):
        return self.ndarray
