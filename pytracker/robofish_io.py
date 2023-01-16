from biotracker import *

import robofish.io
import numpy as np
import matplotlib.pyplot as plt
import sys

class Recorder():
    def __init__(self, world_size_cm=(100, 100), resolution=(2048,2048), hz=25.0):
        self.message_bus = MessageBus()
        self.message_bus.subscribe(Topic.EXPERIMENT_STATE)
        self.message_bus.subscribe(Topic.ENTITIES)
        self.message_bus.subscribe(Topic.SHUTDOWN)
        self.world_size_cm = world_size_cm
        self.resolution = resolution
        self.hz = hz

    def run(self):
        while True:
            msg = self.message_bus.poll(-1)
            if msg.topic == Topic.SHUTDOWN:
                sys.exit(0)
            elif msg.topic == Topic.EXPERIMENT_STATE:
                if msg.experiment_state.recording_state == RecordingState.RECORDING:
                    self.record()

    def record(self):
        sample_count = 0
        recorded_entities = {}

        with robofish.io.File('test.hdf5', mode='w', world_size_cm=self.world_size_cm, frequency_hz=self.hz) as f:
            while True:
                msg = self.message_bus.poll(-1)
                if msg.topic == Topic.SHUTDOWN:
                    sys.exit(0)
                elif msg.topic == Topic.ENTITIES:
                    self.add_observations(msg.entities.entities, recorded_entities, sample_count)
                    sample_count += 1
                elif msg.topic == Topic.EXPERIMENT_STATE:
                    if msg.experiment_state.recording_state != RecordingState.RECORDING:
                        break
            for (id, poses) in recorded_entities.items():
                np_poses = np.array(poses)
                f.create_entity(category='organism', name=id, poses=np_poses)
            if len(recorded_entities.items()) > 0:
                self.plot(f)

    def add_observations(self, observed_entities, recorded_entities, sample_count):
        nan_pose = [np.nan] * 4
        for (id, feature) in observed_entities.items():
            pose = self.feature_to_pose(feature)
            if id not in recorded_entities:
                if sample_count > 0:
                    recorded_entities[id] = [nan_pose] * sample_count
                else:
                    recorded_entities[id] = []
            recorded_entities[id].append(pose)
        for (id, feature) in recorded_entities.items():
            if id not in observed_entities:
                recorded_entities[id].append(nan_pose)

    def feature_to_pose(self, feature):
        front_idx = 0
        middle_idx = 1
        assert(len(feature.nodes) >= 2)
        return [*self.pixel_to_world(feature.nodes[middle_idx].x,
                                     feature.nodes[middle_idx].y), 0, 0]

    def pixel_to_world(self, x, y):
        cm, px = self.world_size_cm, self.resolution
        return [x * cm[0] / px[0] - cm[0] / 2,
                y * cm[1] / px[1] - cm[1] / 2]

    def plot(self, f):
        fig, ax = plt.subplots(1,2, figsize=(10,5))
        f.plot(ax=ax[0])
        f.plot(ax=ax[1], lw_distances=True)
        plt.show()


if __name__ == "__main__":
    recorder = Recorder()
    recorder.run()
