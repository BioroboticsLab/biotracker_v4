from multiprocessing.shared_memory import SharedMemory, resource_tracker
import numpy as np
import zmq

class MessageBus:
    def __init__(self):
        self.context = zmq.Context()
        self.push = self.context.socket(zmq.PUSH)
        self.push.connect("tcp://127.0.0.1:6667")
        self.sub = self.context.socket(zmq.SUB)
        self.sub.connect("tcp://127.0.0.1:6668")

    def subscribe(self, topic):
        self.sub.subscribe(topic)

    def poll(self, timeout):
        if self.sub.poll(timeout) <= 0:
            return None
        msg = self.sub.recv_multipart()
        return (msg[0].decode(), msg[1])

    def send(self, ty, msg):
        self.push.send_multipart([ty.encode(), msg.SerializeToString()])
