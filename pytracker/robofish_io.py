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
                print(poses)
                f.create_entity(category='organism', name=f'fish_{id}', poses=poses)

    def entities_to_numpy(self, track):
        nan_pose = [np.nan] * 4
        last_frame_numbers = {}
        np_entities = {}
        sorted_entities = sorted(track.entities.items(), key=lambda x: x[0])
        for frame_number, entities in sorted_entities:
            for entity in entities.entities:
                pose = entity.feature.pose
                pose = [pose.x_cm, pose.y_cm, pose.orientation_rad, math.degrees(pose.orientation_rad)]
                last_frame_number = last_frame_numbers.get(entity.id, track.start_frame - 1)
                if entity.id not in np_entities:
                    np_entities[entity.id] = []
                if entity.frame_number > last_frame_number + 1:
                    fill_nan_start = last_frame_number + 1
                    fill_nan_end = entity.frame_number
                    np_entities[entity.id].extend([nan_pose] * (fill_nan_end - fill_nan_start))
                np_entities[entity.id].append(pose)
                last_frame_numbers[entity.id] = entity.frame_number
        return {id: np.array(poses) for id, poses in np_entities.items()}

    def plot(self, f, filename):
        os.system('robofish-io-evaluate tracks ' + filename)


async def main():
    addr, port = get_address_and_port()
    server = Server([TrackRecorder()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
