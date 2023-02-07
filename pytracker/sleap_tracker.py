from biotracker import *

import cv2
import numpy as np
import sleap
import json
import math

import asyncio
from grpclib.client import Channel
from grpclib.server import Server

class SLEAPTracker(FeatureDetectorBase):
    async def detect_features(self, request: "DetectorRequest") -> "Features":
        try:
            shared_img = SharedImage(request.image)
        except FileNotFoundError as e:
            raise grpclib.GRPCError(grpclib.const.Status.NOT_FOUND, repr(e))
        buf = shared_img.as_numpy()
        resized = cv2.resize(buf, (self.target_width, self.target_height))
        grayscale = cv2.cvtColor(resized, cv2.COLOR_RGBA2GRAY)
        np_array = grayscale.reshape((1,self.target_width,self.target_height,1)).astype("uint8")
        prediction = self.predictor.inference_model.predict(np_array)
        features = Features(skeleton=self.skeleton)
        for peaks, vals, instance_score in zip(prediction['instance_peaks'][0],
                                               prediction['instance_peak_vals'][0],
                                               prediction['centroid_vals'][0]):
            feature = Feature(score=instance_score)
            for peak, val in zip(peaks, vals):
                scale_x = self.target_width / request.image.width
                scale_y = self.target_width / request.image.width
                node = SkeletonNode(x=peak[0] / scale_x, y=peak[1] / scale_y, score=val)
                feature.nodes.append(node)
            await self.calculate_pose(feature, request.arena, request.image)
            features.features.append(feature)
        return features

    async def set_config(
        self, component_configuration: "ComponentConfiguration"
    ) -> "Empty":
        await self.load_config(component_configuration.config_json)
        return Empty()

    async def load_config(self, config_json):
        config = json.loads(config_json)
        model_paths = config['model_paths']
        self.predictor = sleap.load_model(model_paths, batch_size=1)
        self.target_width = self.predictor.centroid_config.data.preprocessing.target_width
        self.target_height = self.predictor.centroid_config.data.preprocessing.target_height
        await self.initialize_skeleton(config['model_config']['front_node'],
                                       config['model_config']['center_node'])
        # warmup inference
        self.predictor.inference_model.predict(np.zeros((1, self.target_width, self.target_height, 1), dtype = "uint8"))

    async def initialize_skeleton(self, center_node, front_node):
        sleap_skeleton = self.predictor.centroid_config.data.labels.skeletons[0]
        anchor_part = self.predictor.centroid_config.model.heads.centroid.anchor_part
        edges = []
        for from_idx, to_idx in sleap_skeleton.edge_inds:
            edges.append(SkeletonEdge(source=from_idx, target=to_idx))
        for i,name in enumerate(sleap_skeleton.node_names):
            if name == front_node:
                self.front_node_index = i
            if name == center_node:
                self.center_node_index = i
        assert(self.front_node_index is not None and self.center_node_index is not None)
        skeleton_descriptor = SkeletonDescriptor(edges=edges,
                                                 node_names=sleap_skeleton.node_names)
        self.skeleton = skeleton_descriptor

    async def calculate_pose(self, feature, arena, image):
        assert(len(feature.nodes) >= 2)
        center = feature.nodes[self.center_node_index]
        front = feature.nodes[self.front_node_index]
        (x1, y1) = await self.pixel_to_world(center.x, center.y, arena, image)
        (x2, y2) = await self.pixel_to_world(front.x, front.y, arena, image)
        midline = np.array([x2 - x1, y2 - y1])
        direction = midline / np.linalg.norm(midline)
        orientation_rad = np.arctan2(direction[0], direction[1]) + np.pi / 2.0;
        if math.isnan(orientation_rad):
            # happens if center == front
            orientation_rad = 0
        feature.pose = Pose(x_cm=x1, y_cm=y1, orientation_rad=orientation_rad)

    async def pixel_to_world(self, x, y, arena, image):
        return [x * arena.width_cm / image.width - arena.width_cm / 2,
                -(y * arena.height_cm / image.height - arena.height_cm / 2)]

async def main():
    addr, port = get_address_and_port()
    server = Server([SLEAPTracker()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
