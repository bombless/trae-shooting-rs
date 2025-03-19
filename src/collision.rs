use glam::Vec3;

// 墙体碰撞信息结构体
pub struct WallCollider {
    // 墙体的起点和终点坐标
    start: Vec3,
    end: Vec3,
    // 墙体的高度
    height: f32,
    // 墙体的厚度
    thickness: f32,
    // 墙体的法向量（垂直于墙面的方向）
    normal: Vec3,
}

impl WallCollider {
    // 从墙体的起点和终点创建碰撞器
    pub fn new(start: [f32; 3], end: [f32; 3], height: f32, thickness: f32) -> Self {
        // 计算墙体方向和长度
        let dx = end[0] - start[0];
        let dz = end[2] - start[2];
        
        // 计算墙体的法向量（垂直于墙面）
        let length = (dx*dx + dz*dz).sqrt();
        let nx = -dz / length;
        let nz = dx / length;
        
        Self {
            start: Vec3::new(start[0], start[1], start[2]),
            end: Vec3::new(end[0], end[1], end[2]),
            height,
            thickness,
            normal: Vec3::new(nx, 0.0, nz),
        }
    }
    
    // 检测点是否与墙体碰撞
    pub fn check_collision(&self, position: Vec3, radius: f32) -> bool {
        // 如果点的高度超过墙体高度，则不碰撞
        if position.y > self.height {
            return false;
        }
        
        // 计算点到墙体线段的最近点
        let wall_vec = Vec3::new(
            self.end.x - self.start.x,
            0.0,
            self.end.z - self.start.z
        );
        let wall_length_squared = wall_vec.length_squared();
        
        // 计算点到墙体起点的向量
        let point_to_start = Vec3::new(
            position.x - self.start.x,
            0.0,
            position.z - self.start.z
        );
        
        // 计算投影比例（点在墙体线段上的投影位置）
        let t = (point_to_start.dot(wall_vec) / wall_length_squared).clamp(0.0, 1.0);
        
        // 计算墙体线段上的最近点
        let closest_point = Vec3::new(
            self.start.x + t * wall_vec.x,
            0.0,
            self.start.z + t * wall_vec.z
        );
        
        // 计算点到墙体的距离向量
        let distance_vec = Vec3::new(
            position.x - closest_point.x,
            0.0,
            position.z - closest_point.z
        );
        
        // 计算点到墙体的距离
        let distance = distance_vec.length();
        
        // 检查点是否在墙体的两侧
        let dot_product = distance_vec.dot(self.normal);
        
        // 如果点在墙体正面且距离小于半径，或者点在墙体背面且距离小于(半径+墙体厚度)，则发生碰撞
        if (dot_product >= 0.0 && distance < radius) || 
           (dot_product < 0.0 && distance < radius + self.thickness) {
            return true;
        }
        
        false
    }
    
    // 计算碰撞响应（返回调整后的位置）
    pub fn resolve_collision(&self, position: Vec3, radius: f32) -> Vec3 {
        // 如果没有碰撞，直接返回原位置
        if !self.check_collision(position, radius) {
            return position;
        }
        
        // 计算点到墙体线段的最近点
        let wall_vec = Vec3::new(
            self.end.x - self.start.x,
            0.0,
            self.end.z - self.start.z
        );
        let wall_length_squared = wall_vec.length_squared();
        
        // 计算点到墙体起点的向量
        let point_to_start = Vec3::new(
            position.x - self.start.x,
            0.0,
            position.z - self.start.z
        );
        
        // 计算投影比例
        let t = (point_to_start.dot(wall_vec) / wall_length_squared).clamp(0.0, 1.0);
        
        // 计算墙体线段上的最近点
        let closest_point = Vec3::new(
            self.start.x + t * wall_vec.x,
            0.0,
            self.start.z + t * wall_vec.z
        );
        
        // 计算点到墙体的距离向量
        let distance_vec = Vec3::new(
            position.x - closest_point.x,
            0.0,
            position.z - closest_point.z
        );
        
        // 计算点到墙体的距离
        let distance = distance_vec.length();
        
        // 检查点是否在墙体的两侧
        let dot_product = distance_vec.dot(self.normal);
        
        // 根据点在墙体的哪一侧来调整位置
        if dot_product >= 0.0 {
            // 点在墙体正面
            if distance < radius {
                // 计算需要移动的距离
                let move_distance = radius - distance;
                // 沿着距离向量的方向移动
                let move_dir = distance_vec.normalize();
                return position + move_dir * move_distance;
            }
        } else {
            // 点在墙体背面
            if distance < radius + self.thickness {
                // 计算需要移动的距离
                let move_distance = radius + self.thickness - distance;
                // 沿着距离向量的方向移动
                let move_dir = distance_vec.normalize();
                return position + move_dir * move_distance;
            }
        }
        
        position
    }
}

// 创建墙体碰撞器的辅助函数，直接从create_wall函数的参数创建
pub fn create_wall_collider(start: [f32; 3], end: [f32; 3], height: f32) -> WallCollider {
    // 使用与create_wall函数相同的墙体厚度
    let thickness = 0.3; // 30cm thickness
    WallCollider::new(start, end, height, thickness)
}