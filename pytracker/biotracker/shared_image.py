from multiprocessing.shared_memory import SharedMemory, resource_tracker
import numpy as np

class BufferManager:
    def __init__(self):
        self.images = []

    def push(self, image):
        self.images.append(image)
        if len(self.images) > 2:
            self.images = self.images[1:]

    def allocate_image(self, image):
        shared_image = SharedImage(image, create=True)
        self.push(shared_image)
        return shared_image

class SharedImage:
    def __init__(self, img, create=False):
        self.size = img.width * img.height * 4
        if create:
            self.shm = SharedMemory(size=self.size, create=True)
            img.shm_id = self.shm._name
        else:
            self.shm = SharedMemory(img.shm_id, size=self.size, create=False)
            # Don't track this memory, it gets cleaned up by the BioTracker
            resource_tracker.unregister(self.shm._name, 'shared_memory')
        self.ndarray = np.ndarray((img.height, img.width, 4), dtype=np.uint8, buffer=self.shm.buf)
        if not create:
            self.ndarray.flags.writeable = False

    def as_numpy(self):
        return self.ndarray
