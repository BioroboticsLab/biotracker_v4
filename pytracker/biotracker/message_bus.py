from .biotracker.biotracker import Component, BioTrackerMessage, Topic, ComponentMessage
from multiprocessing.shared_memory import SharedMemory, resource_tracker
import numpy as np
import zmq
from enum import Enum

class MessageBus:
    def __init__(self):
        self.context = zmq.Context()
        self.push = self.context.socket(zmq.PUSH)
        self.push.connect("tcp://127.0.0.1:6667")
        self.sub = self.context.socket(zmq.SUB)
        self.sub.connect("tcp://127.0.0.1:6668")

    def register(self, component_descriptor: Component):
        component_message = ComponentMessage(recipient_id="BioTracker", registration=component_descriptor)
        self.send(BioTrackerMessage(topic=Topic.COMPONENT_MESSAGE, component_message=component_message))
        self.sub.subscribe(Topic.COMPONENT_MESSAGE.name + "." + component_descriptor.id)
        msg = self.poll(10000)
        if msg is None:
            raise Exception("No config received")
        if msg.topic != Topic.COMPONENT_MESSAGE:
            raise Exception("Expected config message, received " + msg)
        return msg.component_message.config_json

    def subscribe_images(self, stream_name):
        self.sub.subscribe(Topic.IMAGE.name + "." + stream_name)

    def subscribe(self, topic: Topic):
        self.sub.subscribe(topic.name)

    def poll(self, timeout):
        if self.sub.poll(timeout) <= 0:
            return None
        msg = self.sub.recv_multipart()
        msg = BioTrackerMessage.FromString(msg[1])
        return msg

    def send(self, msg):
        topic = msg.topic.name
        if msg.topic == Topic.IMAGE:
            topic = topic + "." + msg.image.stream_id
        if msg.topic == Topic.COMPONENT_MESSAGE:
            topic = topic + "." + msg.component_message.recipient_id
        self.push.send_multipart([topic.encode(), msg.SerializeToString()])
