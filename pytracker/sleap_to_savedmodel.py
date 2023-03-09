import sleap
import argparse
import numpy as np
import os
import json

parser = argparse.ArgumentParser()
parser.add_argument("-m", "--models", help="list of paths to sleap models",
                    nargs='+', required=True)
parser.add_argument("-s", "--save", help="path to exported SavedModel",
                    default="saved_model", type=str)
args = parser.parse_args()

# Export model as Tensorflow SavedModel
save_path = os.path.abspath(args.save)
predictor = sleap.load_model(args.models, batch_size=1)
target_width = predictor.centroid_config.data.preprocessing.target_width
target_height = predictor.centroid_config.data.preprocessing.target_height
predictor.inference_model.predict(np.zeros((1, target_width, target_height, 1), dtype = "uint8"))
predictor.inference_model.save(save_path)

# Save config file with model metadata
skeleton = predictor.centroid_config.data.labels.skeletons[0]
node_names = skeleton.node_names
edge_indices = skeleton.edge_inds

config = {
    'target_width': target_width,
    'target_height': target_height,
    'node_names': node_names,
    'edge_indices': edge_indices,
}
config_path = os.path.join(save_path, 'config.json')
with open(config_path, 'w') as f:
    json.dump(config, f)

print("Saved model to {}".format(args.save))
print("Add this to the SLEAPTracker section in you biotracker config json:\n")
print(f'''
"model_path": "{save_path}",
''')
