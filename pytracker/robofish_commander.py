import sys
from biotracker import *

import math
import json
import asyncio, socket
from grpclib.server import Server



class RobofishCommanderServerProtocol(asyncio.Protocol):
    def connection_made(self, transport):
        peername = transport.get_extra_info('peername')
        print('Connection from {}'.format(peername))
        self.transport = transport

    def send(self, data):
        try:
            self.transport.write(data)
        except AttributeError:
            pass

class RobofishCommanderBridge(ObserverBase):
    async def set_config(
        self, component_configuration: "ComponentConfiguration"
    ) -> "Empty":
        self.config = json.loads(component_configuration.config_json)
        loop = asyncio.get_running_loop()
        self.protocol = RobofishCommanderServerProtocol()
        self.server = await loop.create_server(
            lambda: self.protocol,
            '127.0.0.1', self.config['port'])
        return Empty()

    async def update(self, experiment: "Experiment") -> "Empty":
        # let mut msg = format!("frame:{frame_number};polygon:0;fishcount:{fishcount};");
        features = experiment.last_features
        fishcount = len(features.features)
        if features is not None:
            msg = f'frame:{features.frame_number};polygon:0;fishcount:{fishcount};'
            timestamp_ms = features.frame_number / experiment.target_fps * 1000.0
            for feature in features.features:
                if feature.pose is not None:
                    orientation_rad = feature.pose.orientation_rad
                    orientation_deg = orientation_rad * 180.0 / math.pi
                    x_cm = feature.pose.x_cm + experiment.arena.width_cm / 2.0
                    y_cm = experiment.arena.height_cm / 2.0 - feature.pose.y_cm
                    msg += f'{feature.id},{x_cm},{y_cm},{orientation_rad},{orientation_deg},20,20,{timestamp_ms},F&'
            if fishcount > 0:
                msg = msg[:-1]
            msg += ";end"
            self.protocol.send(msg.encode())
        return Empty()

async def main():
    heartbeat()
    addr, port = get_address_and_port()
    server = Server([RobofishCommanderBridge()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
