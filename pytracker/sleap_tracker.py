from biotracker import *

import cv2
import numpy as np
import tensorflow as tf
import json
import math
import os
import sys

import asyncio
from grpclib.server import Server

class SLEAPTracker(FeatureDetectorBase):
    async def detect_features(self, request: "DetectorRequest") -> "Features":
        try:
            shared_img = SharedImage(request.image)
        except FileNotFoundError as e:
            raise grpclib.GRPCError(grpclib.const.Status.NOT_FOUND, repr(e))
        buf = shared_img.as_numpy()
        resized = cv2.resize(buf, (self.target_width, self.target_height))
        grayscale = cv2.cvtColor(resized, cv2.COLOR_BGR2GRAY)
        np_array = grayscale.reshape((1,self.target_width,self.target_height,1)).astype("uint8")
        prediction = self.model(np_array)
        features = Features(skeleton=self.skeleton)
        for peaks, vals, instance_score in zip(prediction['instance_peaks'].numpy()[0],
                                               prediction['instance_peak_vals'].numpy()[0],
                                               prediction['centroid_vals'].numpy()[0]):
            feature = Feature(score=instance_score)
            for peak, val in zip(peaks, vals):
                scale_x = self.target_width / request.image.width
                scale_y = self.target_width / request.image.width
                node = SkeletonNode(x=peak[0] / scale_x, y=peak[1] / scale_y, score=val)
                feature.nodes.append(node)
            features.features.append(feature)
        return features

    async def set_config(
        self, component_configuration: "ComponentConfig"
    ) -> "Empty":
        await self.load_config(component_configuration.config_json)
        return Empty()

    async def load_config(self, config_json):
        config = json.loads(config_json)
        model_path = config['model_path']
        config_path = os.path.join(model_path, 'config.json')
        assert(model_path is not None and config_path is not None)
        with open(config_path, 'r') as f:
            metadata = json.load(f)
            self.target_width = metadata['target_width']
            self.target_height = metadata['target_height']
            await self.initialize_skeleton(config['model_config']['front_node'],
                                       config['model_config']['center_node'],
                                       metadata['node_names'],
                                       metadata['edge_indices'])
        self.model = tf.saved_model.load(model_path)

    async def initialize_skeleton(self, center_node, front_node, node_names, edge_indices):
        assert(center_node is not None and front_node is not None)
        assert(node_names is not None and edge_indices is not None)
        edges = []
        for from_idx, to_idx in edge_indices:
            edges.append(SkeletonEdge(source=from_idx, target=to_idx))
        front_node_index = None
        center_node_index = None
        for i,name in enumerate(node_names):
            if name == front_node:
                front_node_index = i
            if name == center_node:
                center_node_index = i
        assert(front_node_index is not None and center_node_index is not None)
        skeleton_descriptor = SkeletonDescriptor(edges=edges,
                                                 node_names=node_names,
                                                 front_index=front_node_index,
                                                 center_index=center_node_index)
        self.skeleton = skeleton_descriptor

async def main():
    heartbeat()
    addr, port = get_address_and_port()
    server = Server([SLEAPTracker()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
