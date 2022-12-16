from .protocol import Message
from multiprocessing.shared_memory import SharedMemory, resource_tracker
import numpy as np
import zmq
import json

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
        msg = json.loads(msg[1])
        return Message(msg=msg).msg

    def send(self, msg):
        ty = msg.type
        self.push.send_multipart([ty.encode(), msg.json().encode()])
