from biotracker import *

import cv2
import numpy as np
import sleap
import json

component_descriptor = Component(id="SLEAPTracker", typ=ComponentType.FEATURE_DETECTOR)

class SLEAPTracker():
    def __init__(self):
        self.message_bus = MessageBus()

    def run(self):
        component_config = self.message_bus.register(component_descriptor)
        self.load_config(component_config)
        self.message_bus.subscribe_images("Tracking")
        self.message_bus.subscribe(Topic.SHUTDOWN)
        while True:
            msg = self.message_bus.poll(-1)
            if msg.topic == Topic.SHUTDOWN:
                break
            if msg.topic == Topic.COMPONENT_MESSAGE:
                self.load_config(msg.component_message.config_json)
            elif msg.topic == Topic.IMAGE:
                img = msg.image
                assert(img.stream_id == "Tracking")
                try:
                    shared_img = SharedImage(img)
                except FileNotFoundError:
                    print(f"Warning: Image '{img.timestamp}' expired (tracking too slow)")
                    # skip to next image
                    continue
                buf = shared_img.as_numpy()
                grayscale = cv2.cvtColor(buf, cv2.COLOR_RGBA2GRAY)
                grayscale = grayscale.reshape((1,self.target_width,self.target_height,1)).astype("uint8")
                prediction = self.predictor.inference_model.predict(grayscale)
                features = Features(timestamp=img.timestamp, skeleton=self.skeleton)
                for peaks, vals, instance_score in zip(prediction['instance_peaks'][0],
                                                        prediction['instance_peak_vals'][0],
                                                        prediction['centroid_vals'][0]):
                    feature = Feature(score=instance_score)
                    for peak, val in zip(peaks, vals):
                        node = SkeletonNode(x=peak[0], y=peak[1], score=val)
                        feature.nodes.append(node)
                    features.features.append(feature)
                self.message_bus.send(BioTrackerMessage(topic=Topic.FEATURES, features=features))
            else:
                print("Unknown message type: " + ty.decode())
                break

    def load_config(self, config_json):
        config = json.loads(config_json)
        model_paths = config['model_paths']
        self.predictor = sleap.load_model(model_paths, batch_size=1)
        self.initialize_skeleton()
        self.target_width = self.predictor.centroid_config.data.preprocessing.target_width
        self.target_height = self.predictor.centroid_config.data.preprocessing.target_height
        # warmup inference
        self.predictor.inference_model.predict(np.zeros((1, self.target_width, self.target_height, 1), dtype = "uint8"))

    def initialize_skeleton(self):
        sleap_skeleton = self.predictor.centroid_config.data.labels.skeletons[0]
        anchor_part = self.predictor.centroid_config.model.heads.centroid.anchor_part
        edges = []
        for from_idx, to_idx in sleap_skeleton.edge_inds:
            edges.append(SkeletonEdge(source=from_idx, target=to_idx))
        skeleton_descriptor = SkeletonDescriptor(edges=edges,
                                                 node_names=sleap_skeleton.node_names,
                                                 center_node=anchor_part)
        self.skeleton = skeleton_descriptor

if __name__ == "__main__":
    tracker = SLEAPTracker()
    tracker.run()
