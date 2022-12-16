from math import isnan
from enum import Enum, auto
from pydantic import BaseModel as PydanticBaseModel, Field, validator
from typing import Union, Literal, List, Dict

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

class Entities(BaseModel):
    type: Literal['Entities']
    pts: int
    entities: dict[int, ImageFeature]

class Message(BaseModel):
    msg: Union[ImageFeatures, ImageData, Entities] = Field(..., discriminator='type')
