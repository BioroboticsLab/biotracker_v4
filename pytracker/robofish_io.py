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

class TrackRecorder(TrackRecorderBase):
    async def set_config(
        self, component_configuration: "ComponentConfiguration"
    ) -> "Empty":
        return Empty()

    async def save(self, track_save_request: "TrackSaveRequest") -> "Empty":
        track = track_save_request.track
        filename = track_save_request.experiment.recording_config.base_path
        experiment = track_save_request.experiment
        self.world_size_cm = (experiment.arena.width_cm,
                              experiment.arena.height_cm)
        self.hz = experiment.target_fps
        recorded_entities = {}
        loop.run_in_executor(None, lambda: self.save_track(filename, track))
        return Empty()

    def save_track(self, filename, track):
        path = filename + '.hdf5'
        with robofish.io.File(filename + '.hdf5', mode='w',
                              world_size_cm=self.world_size_cm,
                              frequency_hz=self.hz) as f:
            for id, poses in self.entities_to_numpy(track).items():
                f.create_entity(category='organism', name=f'fish_{id}', poses=poses)

    def entities_to_numpy(self, track):
        nan_pose = [np.nan] * 4
        entity_last_seen = {}
        np_entities = {}
        sorted_features = sorted(track.features.items(), key=lambda x: x[0])
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
