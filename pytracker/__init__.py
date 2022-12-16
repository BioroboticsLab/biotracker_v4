from typing import Literal, Union
import json

from math import isnan
from enum import Enum, auto
from pydantic import BaseModel as PydanticBaseModel, Field, validator

# JSON specification does not include NaN, so we convert it to None
class BaseModel(PydanticBaseModel):
    @validator('*')
    def change_nan_to_none(cls, v, field):
        if field.outer_type_ is float and isnan(v):
            return None
        return v

class Point(BaseModel):
    x: float
    y: float

class SkeletonNode(BaseModel):
    point: Point
    score: float

class SkeletonEdge(BaseModel):
    source: int
    target: int

class ImageFeature(BaseModel):
    nodes: list[SkeletonNode]
    edges: list[SkeletonEdge]
    score: float

class ImageFeatures(BaseModel):
    type: Literal['Features']
    pts: int
    features: list[ImageFeature]

class ImageData(BaseModel):
    type: Literal['Image']
    pts: int
    shm_id: str
    width: int
    height: int

class Seekable(BaseModel):
    type: Literal['Seekable']
    start: int
    end: int

class Entities(BaseModel):
    type: Literal['Entities']
    pts: int
    entities: dict[int, ImageFeature]

class Message(BaseModel):
    msg: Union[ImageFeatures, ImageData, Seekable, Entities] = Field(..., discriminator='type')

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

from multiprocessing.shared_memory import SharedMemory, resource_tracker
import numpy as np
import zmq

class SharedImage:
    def __init__(self, msg: ImageData):
        self.size = msg.width * msg.height * 4
        self.shm = SharedMemory(msg.shm_id, size=self.size, create=False)
        # Don't track this memory, it gets cleaned up by the BioTracker
        resource_tracker.unregister(self.shm._name, 'shared_memory')
        self.ndarray = np.ndarray((msg.height, msg.width, 4), dtype=np.uint8, buffer=self.shm.buf)

    def as_numpy(self):
        return self.ndarray
