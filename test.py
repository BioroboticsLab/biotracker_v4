import biotracker
from biotracker import *

import cv2
import numpy as np
import sleap

model_paths = [
    "/home/max/projects/sleap/sleap_label_manyfish/models/220926_175742.centered_instance",
    "/home/max/projects/sleap/sleap_label_manyfish/models/220926_175742.centroid"
]

predictor = sleap.load_model(model_paths, batch_size=1)
predictor.inference_model.predict(np.zeros((1, 2048, 2048, 1), dtype = "uint8"))

message_bus = MessageBus()
message_bus.subscribe("Image")
while True:
    msg = message_bus.poll(-1)
    try:
        if msg.type == "Image":
            img = SharedImage(msg)
            buf = img.as_numpy()
            grayscale = cv2.cvtColor(buf, cv2.COLOR_RGBA2GRAY)
            grayscale = grayscale.reshape((1,2048,2048,1)).astype("uint8")
            prediction = predictor.inference_model.predict(grayscale)
            features = []
            for peaks, vals, instance_score in zip(prediction['instance_peaks'][0],
                                                   prediction['instance_peak_vals'][0],
                                                   prediction['centroid_vals'][0]):
                feature = ImageFeature(nodes=[], edges=[], score=instance_score)
                for peak, val in zip(peaks, vals):
                    point = Point(x=peak[0], y=peak[1])
                    node = SkeletonNode(point=point, score=val)
                    feature.nodes.append(node)
                features.append(feature)
            message_bus.send(ImageFeatures(type="Features", pts=msg.pts, features=features))
    except Exception as e:
        print(e)
