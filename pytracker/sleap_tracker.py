from biotracker import *

import cv2
import numpy as np
import sleap
import json

model_paths = [
    "/home/max/projects/sleap/sleap_label_manyfish/models/220926_175742.centered_instance",
    "/home/max/projects/sleap/sleap_label_manyfish/models/220926_175742.centroid"
]

class SLEAPTracker():
    def __init__(self):
        self.message_bus = MessageBus()
        self.message_bus.subscribe(Topic.IMAGE)
        self.message_bus.subscribe(Topic.SHUTDOWN)
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
        skeleton_msg = ExperimentUpdate(skeleton_descriptor=skeleton_descriptor)
        self.message_bus.send(BioTrackerMessage(topic=Topic.EXPERIMENT_UPDATE,
                                                experiment_update=skeleton_msg))

    def run(self):
        while True:
            msg = self.message_bus.poll(-1)
            if msg.topic == Topic.SHUTDOWN:
                break
            elif msg.topic == Topic.IMAGE:
                img = msg.image
                if img.stream_id != "Tracking":
                    continue
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
                features = Features(timestamp=img.timestamp)
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

if __name__ == "__main__":
    tracker = SLEAPTracker()
    tracker.run()
