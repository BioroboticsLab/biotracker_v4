import sys
from biotracker import *

import robofish.io
import numpy as np
import matplotlib.pyplot as plt
import sys
import json
import os
import math

import asyncio
from grpclib.client import Channel
from grpclib.server import Server

class TrackRecorder(ObserverBase):
    def __init__(self):
        self.recording_state = RecordingState.INITIAL

    async def set_config(
        self, component_configuration: "ComponentConfiguration"
    ) -> "Empty":
        return Empty()

    async def update(self, experiment: "Experiment") -> "Empty":
        if experiment.recording_state != self.recording_state:
            if experiment.recording_state == RecordingState.RECORDING:
                await self.start_recording(experiment)
            elif experiment.recording_state == RecordingState.FINISHED:
                await self.stop_recording(experiment)
            self.recording_state = experiment.recording_state
        if self.recording_state == RecordingState.RECORDING:
            features = experiment.last_features
            self.track.features[features.frame_number] = features
        return Empty()


    async def start_recording(self, experiment):
        self.track = Track(features={},skeleton=experiment.skeleton)
        self.filename = experiment.recording_config.base_path + '.hdf5'
        self.world_size_cm = (experiment.arena.width_cm,
                              experiment.arena.height_cm)
        self.hz = experiment.target_fps

    async def stop_recording(self, experiment):
        with robofish.io.File(self.filename, mode='w',
                              world_size_cm=self.world_size_cm,
                              frequency_hz=self.hz) as f:
            for id, poses in self.entities_to_numpy().items():
                f.create_entity(category='organism', name=f'fish_{id}', poses=poses)

    def entities_to_numpy(self):
        nan_pose = [np.nan] * 4
        entity_last_seen = {}
        np_entities = {}
        sorted_features = sorted(self.track.features.items(), key=lambda x: x[0])
        for frame_number, features in sorted_features:
            for feature in features.features:
                if feature.id is None:
                    continue
                pose = feature.pose
                pose = [pose.x_cm, pose.y_cm, pose.orientation_rad, math.degrees(pose.orientation_rad)]
                last_seen = entity_last_seen.get(feature.id, -1)
                if feature.id not in np_entities:
                    np_entities[feature.id] = []
                if frame_number > last_seen + 1:
                    fill_nan_start = last_seen + 1
                    fill_nan_end = frame_number
                    np_entities[feature.id].extend([nan_pose] * (fill_nan_end - fill_nan_start))
                np_entities[feature.id].append(pose)
                entity_last_seen[feature.id] = frame_number
        return {id: np.array(poses) for id, poses in np_entities.items()}

    def plot(self, filename):
        os.system('robofish-io-evaluate tracks ' + filename)


async def main():
    heartbeat()
    addr, port = get_address_and_port()
    server = Server([TrackRecorder()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
