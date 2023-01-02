from .tracking_pb2 import Image
from multiprocessing.shared_memory import SharedMemory, resource_tracker
import numpy as np

class SharedImage:
    def __init__(self, img: Image):
        self.size = img.width * img.height * 4
        self.shm = SharedMemory(img.shm_id, size=self.size, create=False)
        # Don't track this memory, it gets cleaned up by the BioTracker
        resource_tracker.unregister(self.shm._name, 'shared_memory')
        self.ndarray = np.ndarray((img.height, img.width, 4), dtype=np.uint8, buffer=self.shm.buf)
        self.ndarray.flags.writeable = False

    def as_numpy(self):
        return self.ndarray
