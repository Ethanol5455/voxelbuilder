-- Noise Parameters
dividend = 0.10
-- Generation Parameters
max_build_height = 16 * 16
max_gen_height = 90 -- Max 250
min_gen_height = 60 -- Min 10

-- Flora Parameters 
tree_prob = 500 -- 1 in treeProb
tree_log_min_height = 4
tree_log_max_height = 8
grass_prob = 50 -- 1 in grassProb
rose_prob = 50 -- 1 in roseProb

-- Get Needed IDs
air_id = get_id_by_name("Air")
bedrock_id = get_id_by_name("Bedrock")
stone_id = get_id_by_name("Stone")
dirt_id = get_id_by_name("Dirt")
grass_id = get_id_by_name("Grass")
log_id = get_id_by_name("Log")
leaves_id = get_id_by_name("Leaves")
top_grass_id = get_id_by_name("TopGrass")
rose_id = get_id_by_name("Rose")

set_layers(0, 0, bedrock_id)
set_layers(1, min_gen_height - 10, stone_id)

for x = 0, 15, 1
do
    for z = 0, 15, 1
    do
        noise_val = get_noise_2d("OpenSimplex2", column_x / dividend + (x / 16.0) / dividend, column_z / dividend + (z / 16.0) / dividend)
        noise_val = (noise_val + 1.0) / 2.0;

        height_offset = math.floor((max_gen_height - 1 - min_gen_height) * noise_val)

        top_height = min_gen_height + height_offset

        -- Terrain Generation
        set_block(x, top_height, z, grass_id)
        for y = top_height - 1, top_height - 6, -1
        do
            set_block(x, y, z, dirt_id)
        end
        for y = top_height - 7, min_gen_height -9, -1
        do
            set_block(x, y, z, stone_id)
        end

        if(random() % tree_prob == 0)
        then
            tree_height = random() % (tree_log_max_height - tree_log_min_height) + tree_log_min_height
            set_block(x, top_height + tree_height + 2, z, leaves_id)
            for xt = -2, 2, 1
            do
                for zt = -2, 2, 1
                do
                    set_block(x + xt, top_height + tree_height + 1, z + zt, leaves_id)
                end
            end

            for y = top_height + 1, top_height + tree_height + 1, 1
            do
                set_block(x, y, z, log_id)
            end
            set_block(x, top_height, z, dirt_id)
        elseif(random() % grass_prob == 0)
        then
            set_block(x, top_height + 1, z, top_grass_id)
        elseif(random() % rose_prob == 0)
        then
            set_block(x, top_height + 1, z, rose_id)
        end
    end
end
