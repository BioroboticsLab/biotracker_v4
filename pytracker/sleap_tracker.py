from biotracker.message_bus import MessageBus
from biotracker.shared_image import SharedImage

import tracking_pb2 as tracking
import experiment_pb2 as experiment

import cv2
import numpy as np
import sleap
import json
# import networkx as nx

message_bus = MessageBus()
message_bus.subscribe("IMAGE")
message_bus.subscribe("SHUTDOWN")

model_paths = [
    "/home/max/projects/sleap/sleap_label_manyfish/models/220926_175742.centered_instance",
    "/home/max/projects/sleap/sleap_label_manyfish/models/220926_175742.centroid"
]
predictor = sleap.load_model(model_paths, batch_size=1)

# load skeleton descriptor
def read_skeleton(predictor):
    sleap_skeleton = predictor.centroid_config.data.labels.skeletons[0]
    anchor_part = predictor.centroid_config.model.heads.centroid.anchor_part
    edges = []
    for from_idx, to_idx in sleap_skeleton.edge_inds:
        edges.append(tracking.SkeletonDescriptor.SkeletonEdge(source=from_idx, target=to_idx))
    skeleton_descriptor = tracking.SkeletonDescriptor(edges=edges,
                                                      node_names=sleap_skeleton.node_names,
                                                      center_node=anchor_part)
    return skeleton_descriptor
skeleton_descriptor = read_skeleton(predictor)
skeleton_msg = experiment.ExperimentUpdate(skeleton_descriptor=skeleton_descriptor)
message_bus.send("EXPERIMENT_UPDATE", skeleton_msg)

# warmup inference
target_width = predictor.centroid_config.data.preprocessing.target_width
target_height = predictor.centroid_config.data.preprocessing.target_height
predictor.inference_model.predict(np.zeros((1, target_width, target_height, 1), dtype = "uint8"))

while True:
    (typ, msg) = message_bus.poll(-1)
    if typ == "SHUTDOWN":
        break
    elif typ == "IMAGE":
        img = tracking.Image.FromString(msg)
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
        grayscale = grayscale.reshape((1,target_width,target_height,1)).astype("uint8")
        prediction = predictor.inference_model.predict(grayscale)
        features = tracking.Features(timestamp=img.timestamp)
        for peaks, vals, instance_score in zip(prediction['instance_peaks'][0],
                                                prediction['instance_peak_vals'][0],
                                                prediction['centroid_vals'][0]):
            feature = tracking.Feature(score=instance_score)
            for peak, val in zip(peaks, vals):
                node = tracking.Feature.SkeletonNode(x=peak[0], y=peak[1], score=val)
                feature.nodes.append(node)
            features.features.append(feature)
        message_bus.send("FEATURES", features)
    else:
        print("Unknown message type: " + ty.decode())
        break
