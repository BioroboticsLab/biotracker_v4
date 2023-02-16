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

    async def load(self, empty: "Empty") -> "Track":
        raise grpclib.GRPCError(grpclib.const.Status.UNIMPLEMENTED)

    async def save(self, track_save_request: "TrackSaveRequest") -> "Empty":
        tracks = track_save_request.tracks
        filename = track_save_request.save_path
        experiment = track_save_request.experiment
        self.world_size_cm = (experiment.arena.width_cm,
                              experiment.arena.height_cm)
        self.hz = experiment.target_fps
        recorded_entities = {}

        loop.run_in_executor(None, lambda: self.save_tracks(filename, tracks))
        return Empty()

    def save_tracks(self, filename, tracks):
        tracks, start_frame_number = self.preprocess_tracks(tracks)
        path = filename + '.hdf5'
        with robofish.io.File(path, mode='w', world_size_cm=self.world_size_cm, frequency_hz=self.hz) as f:
            for id, (track, skeleton) in tracks.items():
                (poses,outlines) = self.track_to_poses(track, start_frame_number, skeleton)
                f.create_entity(category='organism', name=f'fish_{id}', poses=poses)

        if len(tracks) > 0:
            self.plot(f, path)

    def preprocess_tracks(self, tracks):
        processed = {}
        min_frame_number = 2**32
        for id, track in tracks.items():
            skeleton = track.skeleton
            track = sorted(track.observations.items(), key=lambda x: x[0])
            min_frame_number = min(min_frame_number, track[0][0])
            processed[id] = (track, skeleton)
        return (processed, min_frame_number)

    def track_to_poses(self, track, start_frame_number, skeleton):
        poses = []
        skeletons = []
        nan_pose = [np.nan] * 4
        last_frame_number = start_frame_number - 1
        n_nodes = len(skeleton.node_names)
        nan_nodes = [[np.nan, np.nan] for _ in range(n_nodes)]
        for frame_number, entity in track:
            pose = entity.feature.pose

            pose = [pose.x_cm, pose.y_cm, pose.orientation_rad, math.degrees(pose.orientation_rad)]
            if entity.frame_number > last_frame_number + 1:
                fill_nan_start = last_frame_number + 1
                fill_nan_end = entity.frame_number
                poses.extend([nan_pose] * (fill_nan_end - fill_nan_start))
                skeletons.extend([nan_nodes] * (fill_nan_end - fill_nan_start))
            nodes = []
            for node in entity.feature.nodes:
                nodes.append([node.x, node.y])
            skeletons.append(nodes)
            poses.append(pose)
            last_frame_number = entity.frame_number
        return (np.array(poses), np.array(skeletons))

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
